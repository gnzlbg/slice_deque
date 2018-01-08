use super::*;

use core::nonzero::NonZero;

const MAX_NO_ITERS: usize = 3;

/// Number of required memory pages to hold `bytes`.
fn no_required_pages(bytes: usize) -> usize {
    let r = (bytes / page_size()).max(1);
    if r % 2 == 0 {
        r
    } else {
        r + 1
    }
}

/// Mirrored Buffer
pub struct Buffer<T> {
    ptr: NonZero<*mut T>,
    size: usize,
}

impl<T> Buffer<T> {
    /// Number of elements in the buffer.
    pub fn size(&self) -> usize {
        self.size
    }

    pub unsafe fn ptr(&self) -> NonZero<*mut T> {
        self.ptr
    }

    /// Reinterpret contents as a slice.
    ///
    /// Some memory might be uninitialized.
    pub unsafe fn as_slice(&self) -> &[T] {
        ::std::slice::from_raw_parts(self.ptr.get(), self.size())
    }

    /// Reinterpret contents as a mut slice.
    ///
    /// Some memory might be uninitialized.
    pub unsafe fn as_mut_slice(&mut self) -> &mut [T] {
        ::std::slice::from_raw_parts_mut(self.ptr.get(), self.size())
    }

    /// Reinterpret content as a slice and access the `i`-th element.
    ///
    /// The memory of the `i`-th element might be uninitialized.
    pub unsafe fn get(&self, i: usize) -> &T {
        &self.as_slice()[i]
    }

    /// Reinterpret content as a mut slice and access the `i`-th element.
    ///
    /// The memory of the `i`-th element might be uninitialized.
    pub unsafe fn get_mut(&mut self, i: usize) -> &mut T {
        &mut self.as_mut_slice()[i]
    }

    /// Creates a new empty Buffer.
    pub fn new() -> Buffer<T> {
        // Zero-sized elements are not supported yet:
        assert!(::std::mem::size_of::<T>() > 0);
        // Here `ptr` is initialized to a magic value but `size == 0`
        // will ensure that it is never dereferenced in this state.
        unsafe {
            Buffer {
                ptr: NonZero::new_unchecked(::std::usize::MAX as *mut T),
                size: 0,
            }
        }
    }

    /// Create a mirrored buffer containing `size` `T`s where the first half of
    /// the buffer is mirrored into the second half.
    pub unsafe fn uninitialized(size: usize) -> Result<Buffer<T>, ()> {
        //println!("Allocating an uninitialized buffer:");
        //println!("  - element size: {} [bytes]", ::std::mem::size_of::<T>());
        //println!(
        //    "  - element alignment: {} [bytes]",
        //   ::std::mem::align_of::<T>()
        //);
        //println!("  - page size: {} [bytes]", page_size());
        //println!("  - #elements: {} [T]", size);

        // The alignment requirements of `T` must be smaller than the page-size
        // and the page-size must be a multiple of `T` (to be able to mirror the
        // buffer without wholes).
        assert!(::std::mem::align_of::<T>() <= page_size());
        assert!(page_size() % ::std::mem::size_of::<T>() == 0);
        // To split the buffer in two halfs the number of elements must be a
        // multiple of two, and greater than zero to be able to mirror something.
        if size == 0 {
            return Ok(Self::new());
        }
        assert!(size % 2 == 0);

        // How much memory we need:
        let alloc_size = no_required_pages(size * ::std::mem::size_of::<T>()) * page_size();
        //println!("  - #bytes allocated: {} [bytes]", alloc_size);
        debug_assert!(alloc_size > 0);
        debug_assert!(alloc_size % 2 == 0);
        debug_assert!(alloc_size % page_size() == 0);
        let half_alloc_size = alloc_size / 2;

        // 2. We allocate twice the memory, deallocate the second half, and
        // remap the first half into the second which is free for a short
        // period of time. If the remapping fails, we deallocate the first
        // half, and try again.
        let mut no_iters = 0;
        let ptr = loop {
            if no_iters > MAX_NO_ITERS {
                panic!("number of iterations exceeded!");
            }

            //println!("  - 1. Initial allocation...");
            // If the first allocation fails we are done:
            let ptr = alloc(alloc_size)?;
            //println!("  - 1. Initial allocation succeeded: {:?}", ptr);

            // This cannot overflow isize: the worst case alloc_size is
            // usize::MAX, where alloc_size / 2 == isize::MAX.
            let ptr_2nd_half = ptr.offset(half_alloc_size as isize);
            dealloc(ptr_2nd_half, half_alloc_size).map_err(|()| {
                // If deallocating the second half fails we deallocate
                // everything and fail:
                if dealloc(ptr, alloc_size).is_err() {
                    // If deallocating everything also fails returning an
                    // Error would leak memory so panic:
                    panic!("failed to deallocate the 2nd half and then failed to clean up");
                }
                ()
            })?;
            //println!(
            //    "  - 2. dealloc succeeded | ptr: {:?} | ptr_2nd_half: {:?}",
            //    ptr,
            //    ptr_2nd_half
            //);

            // Remap the first half into the second half:
            if remap(ptr, ptr_2nd_half, half_alloc_size).is_ok() {
                // If this succeeds, we are done:
                //println!("  - 3. remap succeeded: {:?}", ptr);
                break ptr;
            }
            //println!("  - remap failed !");

            // Otherwise, we deallocate everything and try again:
            if dealloc(ptr, half_alloc_size).is_err() {
                // If deallocating everything also fails returning an
                // Error would leak memory so panic:
                panic!("failed to deallocate the 2nd half and then failed to clean up");
            }
            //println!("  - dealloc succeded, retrying...");
            no_iters += 1;
        };
        Ok(Buffer {
            ptr: NonZero::new_unchecked(ptr as *mut T),
            size: alloc_size / ::std::mem::size_of::<T>(),
        })
    }
}

impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        if self.size == 0 {
            return;
        }

        // FIXME ? On "darwin" we can deallocate the non-mirrored and mirrored
        // parts of the buffer at once:
        let buffer_size_in_bytes = self.size() * ::std::mem::size_of::<T>();
        let ptr_first_half = self.ptr.get() as *mut u8;
        // If deallocation fails while calling drop we just panic:
        dealloc(ptr_first_half, buffer_size_in_bytes).unwrap()
    }
}

impl<T> Clone for Buffer<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        unsafe {
            let mut c = Buffer::uninitialized(self.size).unwrap();
            let (from, _) = self.as_slice().split_at(self.size() / 2);
            {
                let (to, _) = c.as_mut_slice().split_at_mut(self.size() / 2);
                to[..self.size() / 2].clone_from_slice(&from[..self.size() / 2]);
            }
            c
        }
    }
}

impl<T> Default for Buffer<T> {
    fn default() -> Self {
        Buffer::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let a = Buffer::<u64>::new();
        assert!(a.size() == 0);
    }

    fn test_alloc(size: usize) {
        unsafe {
            let mut a = Buffer::<u64>::uninitialized(size).unwrap();
            let sz = a.size();
            assert!(sz >= size);
            assert_eq!(
                sz,
                no_required_pages(size * ::std::mem::size_of::<u64>()) * page_size()
                    / ::std::mem::size_of::<u64>()
            );

            for i in 0..sz / 2 {
                *a.get_mut(i) = i as u64;
            }

            let (first_half_mut, second_half_mut) = a.as_mut_slice().split_at_mut(sz / 2);

            let mut c = 0;
            for (i, j) in first_half_mut.iter().zip(second_half_mut) {
                assert_eq!(i, j);
                c += 1;
            }
            assert_eq!(c, sz / 2);
        }
    }

    #[test]
    fn allocations() {
        let elements_per_page = page_size() / ::std::mem::size_of::<u64>();
        let sizes = [
            8,
            elements_per_page / 2,
            elements_per_page,
            elements_per_page * 4,
        ];
        for &i in &sizes {
            test_alloc(i);
        }
    }

    #[test]
    fn no_pages_required() {
        // Up to the page size we always need two memory pages
        assert_eq!(no_required_pages(page_size() / 4), 2);
        assert_eq!(no_required_pages(page_size() / 2), 2);
        assert_eq!(no_required_pages(page_size()), 2);
        assert_eq!(no_required_pages(2 * page_size()), 2);
        // After the page sizes we always round up to the next even number of pages:
        assert_eq!(no_required_pages(3 * page_size()), 4);
        assert_eq!(no_required_pages(4 * page_size()), 4);
        assert_eq!(no_required_pages(5 * page_size()), 6);
    }
}
