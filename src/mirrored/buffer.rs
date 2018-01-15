//! Implements a mirrored memory buffer.

use super::*;

use core::nonzero::NonZero;

/// Maximum number of allocation iterations.
///
/// The memory allocation algorithm is:
/// - 1. allocate memory for 2x the buffer
/// - 2. deallocate the second half
/// - 3. mirror the first half into the second half
///
/// There is a race between steps 2 and 3: if after step 2. another process
/// allocates memory in the mean time, and the OS gives it virtual addresses in
/// the second half, then step 3 will fail.
///
/// If that happens, we try again. This constant specifies the maximum number
/// of times that we will try.
#[cfg(any(target_os = "linux", target_os = "macos"))]
const MAX_NO_ALLOC_ITERS: usize = 3;

#[cfg(target_os = "windows")]
const MAX_NO_ALLOC_ITERS: usize = 5;

/// Number of required memory allocation units to hold `bytes`.
fn no_required_allocation_units(bytes: usize) -> usize {
    let r = (bytes / allocation_granularity()).max(1);
    if r % 2 == 0 {
        r
    } else {
        r + 1
    }
}

/// Mirrored memory buffer of length `len`.
///
/// The buffer elements in range `[0, len/2)` are mirrored into the range
/// `[len/2, len)`.
pub struct Buffer<T> {
    /// Pointer to the first element in the buffer.
    ptr: NonZero<*mut T>,
    /// Length of the buffer:
    ///
    /// * it is always a multiple of 2
    /// * the elements in range `[0, len/2)` are mirrored into the range
    /// `[len/2, len)`.
    len: usize,
}

impl<T> Buffer<T> {
    /// Number of elements in the buffer.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Is the buffer empty?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Pointer to the first element in the buffer.
    pub unsafe fn ptr(&self) -> NonZero<*mut T> {
        self.ptr
    }

    /// Interprets contents as a slice.
    ///
    /// Warning: Some memory might be uninitialized.
    pub unsafe fn as_slice(&self) -> &[T] {
        ::std::slice::from_raw_parts(self.ptr.get(), self.len())
    }

    /// Interprets contents as a mut slice.
    ///
    /// Warning: Some memory might be uninitialized.
    pub unsafe fn as_mut_slice(&mut self) -> &mut [T] {
        ::std::slice::from_raw_parts_mut(self.ptr.get(), self.len())
    }

    /// Interprets content as a slice and access the `i`-th element.
    ///
    /// Warning: The memory of the `i`-th element might be uninitialized.
    pub unsafe fn get(&self, i: usize) -> &T {
        &self.as_slice()[i]
    }

    /// Interprets content as a mut slice and access the `i`-th element.
    ///
    /// Warning: The memory of the `i`-th element might be uninitialized.
    pub unsafe fn get_mut(&mut self, i: usize) -> &mut T {
        &mut self.as_mut_slice()[i]
    }

    /// Creates a new empty `Buffer`.
    pub fn new() -> Self {
        // Zero-sized elements are not supported yet:
        assert!(::std::mem::size_of::<T>() > 0);
        // Here `ptr` is initialized to a magic value but `len == 0`
        // will ensure that it is never dereferenced in this state.
        unsafe {
            Self {
                ptr: NonZero::new_unchecked(::std::usize::MAX as *mut T),
                len: 0,
            }
        }
    }

    /// Creates a new empty `Buffer` from a `ptr` and a `len`.
    ///
    /// # Panics
    ///
    /// If `ptr` is null.
    pub unsafe fn from_raw_parts(ptr: *mut T, len: usize) -> Self {
        // Zero-sized types are not supported yet:
        assert!(::std::mem::size_of::<T>() > 0);
        assert!(!ptr.is_null());
        Self {
            ptr: NonZero::new_unchecked(ptr),
            len: len,
        }
    }

    /// Total number of bytes in the buffer (including mirrored memory).
    fn size_in_bytes(len: usize) -> usize {
        no_required_allocation_units(len * ::std::mem::size_of::<T>())
            * allocation_granularity()
    }

    /// Create a mirrored buffer containing `len` `T`s where the first half of
    /// the buffer is mirrored into the second half.
    pub unsafe fn uninitialized(len: usize) -> Result<Self, ()> {
        // Zero-sized types are not supported yet:
        assert!(::std::mem::size_of::<T>() > 0);
        // The alignment requirements of `T` must be smaller than the
        // allocation granularity.
        assert!(::std::mem::align_of::<T>() <= allocation_granularity());
        // To split the buffer in two halfs the number of elements must be a
        // multiple of two, and greater than zero to be able to mirror
        // something.
        if len == 0 {
            return Ok(Self::new());
        }
        assert!(len % 2 == 0);

        // How much memory we need:
        let alloc_size = Self::size_in_bytes(len);
        debug_assert!(alloc_size > 0);
        debug_assert!(alloc_size % 2 == 0);
        debug_assert!(alloc_size % allocation_granularity() == 0);

        Self::allocate_uninitialized(alloc_size)
    }

    /// Allocates an uninitialzied buffer that holds `alloc_size` bytes, where
    /// the bytes in range `[0, alloc_size / 2)` are mirrored into the bytes in
    /// range `[alloc_size / 2, alloc_size)`.
    ///
    /// On Linux and Macos X the algorithm is as follows:
    ///
    /// * 1. Allocate twice the memory (`alloc_size` bytes)
    /// * 2. Deallocate the second half (bytes in range `[alloc_size / 2, 0)`)
    /// * 3. Race condition: mirror bytes of the first half into the second
    /// half.
    ///
    /// If we get a race (e.g. because some other process allocates to the
    /// second half) we release all the resources (we need to deallocate the
    /// memory) and try again (up to a maximum of `MAX_NO_ALLOC_ITERS` times).
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    unsafe fn allocate_uninitialized(alloc_size: usize) -> Result<Self, ()> {
        let half_alloc_size = alloc_size / 2;

        let mut no_iters = 0;
        let ptr = loop {
            if no_iters > MAX_NO_ALLOC_ITERS {
                panic!("number of iterations exceeded!");
            }

            // If the first allocation fails we are done:
            let ptr = alloc(alloc_size)?;

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

            // Mirror the first half into the second half:
            if mirror(ptr, ptr_2nd_half, half_alloc_size).is_ok() {
                // If this succeeds, we are done:
                break ptr;
            }

            // Otherwise, we deallocate everything and try again:
            if dealloc(ptr, half_alloc_size).is_err() {
                // If deallocating everything also fails returning an
                // Error would leak memory so panic:
                panic!("failed to deallocate the 2nd half and then failed to clean up");
            }
            no_iters += 1;
        };

        Ok(Self {
            ptr: NonZero::new_unchecked(ptr as *mut T),
            len: alloc_size / ::std::mem::size_of::<T>(),
        })
    }

    /// Allocates an uninitialzied buffer that holds `alloc_size` bytes, where
    /// the bytes in range `[0, alloc_size / 2)` are mirrored into the bytes in
    /// range `[alloc_size / 2, alloc_size)`.
    ///
    /// On Windows the algorithm is as follows:
    ///
    /// * 1. Allocate physical memory to hold `alloc_size / 2` bytes using a
    ///   memory mapped file.
    /// * 2. Find a region of virtual memory large enough to hold `alloc_size`
    /// bytes (by allocating memory with `VirtualAlloc` and immediately
    /// freeing   it with `VirtualFree`).
    /// * 3. Race condition: map the physical memory to the two halves of the
    ///   virtual memory region.
    ///
    /// If we get a race (e.g. because some other process obtains memory in the
    /// memory region where we wanted to map our physical memory) we release
    /// the first portion of virtual memory if mapping succeeded and try
    /// again (up to a maximum of `MAX_NO_ALLOC_ITERS` times).
    #[cfg(target_os = "windows")]
    unsafe fn allocate_uninitialized(alloc_size: usize) -> Result<Self, ()> {
        let half_alloc_size = alloc_size / 2;

        let file_mapping = create_file_mapping(half_alloc_size)?;

        let mut no_iters = 0;
        let virt_ptr = loop {
            if no_iters > MAX_NO_ALLOC_ITERS {
                // If we exceeded the number of iterations try to close the
                // handle and panic:
                close_file_mapping(file_mapping)
                    .expect("freeing physical memory failed");
                panic!("number of iterations exceeded!");
            }

            // Find large enough virtual memory region (if this fails we are
            // done):
            let virt_ptr = reserve_virtual_memory(alloc_size)?;

            // Map the physical memory to the first half:
            if map_file_to_memory(file_mapping, half_alloc_size, virt_ptr)
                .is_err()
            {
                // If this fails, there is nothing to free and we try again:
                no_iters += 1;
                continue;
            }

            // Map physical memory to the second half:
            if map_file_to_memory(
                file_mapping,
                half_alloc_size,
                virt_ptr.offset(half_alloc_size as isize),
            ).is_err()
            {
                // If this fails, we release the map of the first half and try
                // again:
                no_iters += 1;
                if unmap_file_from_memory(virt_ptr).is_err() {
                    // If unmapping fails try to close the handle and
                    // panic:
                    close_file_mapping(file_mapping)
                        .expect("freeing physical memory failed");
                    panic!("unmapping first half of memory failed")
                }
                continue;
            }

            // We are done
            break virt_ptr;
        };

        // Close the file handle, it will be released when all the memory is
        // unmapped:
        close_file_mapping(file_mapping)
            .expect("closing file handle failed");


        Ok(Self {
            ptr: NonZero::new_unchecked(virt_ptr as *mut T),
            len: alloc_size / ::std::mem::size_of::<T>(),
        })
    }
}

impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        if self.is_empty() {
            return;
        }

        let buffer_size_in_bytes = Self::size_in_bytes(self.len());
        let first_half_ptr = self.ptr.get() as *mut u8;

        // On "macos" we can deallocate the non-mirrored and mirrored parts of
        // the buffer at once:
        #[cfg(target_os = "macos")] {
            // If deallocation fails while calling drop we just panic:
            dealloc(first_half_ptr, buffer_size_in_bytes)
                .expect("deallocating mirrored buffer failed")
        }

        // On linux "linux" 
        #[cfg(target_os = "linux")] {
            let half_alloc_size = buffer_size_in_bytes / 2;
            let second_half_ptr = unsafe {
                first_half_ptr.offset(half_alloc_size as isize)
            };
            // If deallocation fails while calling drop we just panic:
            dealloc(first_half_ptr, buffer_size_in_bytes)
                .expect("deallocating first buffer failed")
        }

        // On "windows" we unmap the memory.
        #[cfg(target_os = "windows")] {
            let half_alloc_size = buffer_size_in_bytes / 2;
            unmap_file_from_memory(first_half_ptr)
                .expect("unmapping first buffer half failed");
            let second_half_ptr = unsafe {
                first_half_ptr.offset(half_alloc_size as isize)
            };
            unmap_file_from_memory(second_half_ptr)
                .expect("unmapping second buffer half failed");
        }
    }
}

impl<T> Clone for Buffer<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        unsafe {
            let mid = self.len() / 2;
            let mut c = Self::uninitialized(self.len())
                .expect("allocating a new mirrored buffer failed");
            let (from, _) = self.as_slice().split_at(mid);
            {
                let (to, _) = c.as_mut_slice().split_at_mut(mid);
                to[..mid].clone_from_slice(&from[..mid]);
            }
            c
        }
    }
}

impl<T> Default for Buffer<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let a = Buffer::<u64>::new();
        assert!(a.is_empty());
    }

    fn test_alloc(size: usize) {
        unsafe {
            let mut a = Buffer::<u64>::uninitialized(size).unwrap();
            let sz = a.len();
            assert!(sz >= size);
            assert_eq!(
                sz,
                Buffer::<u64>::size_in_bytes(size)
                    / ::std::mem::size_of::<u64>()
            );

            for i in 0..sz / 2 {
                *a.get_mut(i) = i as u64;
            }

            let (first_half_mut, second_half_mut) =
                a.as_mut_slice().split_at_mut(sz / 2);

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
        let elements_per_alloc_unit =
            allocation_granularity() / ::std::mem::size_of::<u64>();
        let sizes = [
            8,
            elements_per_alloc_unit / 2,
            elements_per_alloc_unit,
            elements_per_alloc_unit * 4,
        ];
        for &i in &sizes {
            test_alloc(i);
        }
    }

    #[test]
    fn no_alloc_units_required() {
        // Up to the allocation unit size we always need two allocation units
        assert_eq!(
            no_required_allocation_units(allocation_granularity() / 4),
            2
        );
        assert_eq!(
            no_required_allocation_units(allocation_granularity() / 2),
            2
        );
        assert_eq!(no_required_allocation_units(allocation_granularity()), 2);
        assert_eq!(
            no_required_allocation_units(2 * allocation_granularity()),
            2
        );
        // For sizes larger than the allocation units we always round up to the
        // next even number of allocation units:
        assert_eq!(
            no_required_allocation_units(3 * allocation_granularity()),
            4
        );
        assert_eq!(
            no_required_allocation_units(4 * allocation_granularity()),
            4
        );
        assert_eq!(
            no_required_allocation_units(5 * allocation_granularity()),
            6
        );
    }
}
