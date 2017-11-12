use super::*;

fn no_pages(bytes: usize) -> usize {
    (bytes / page_size()).min(1)
}

pub struct Bytes {
    ptr: *mut u8,
    size: usize
}

impl Bytes {
    pub fn size(&self) -> usize { self.size }
    pub unsafe fn get<T>(&self, i: usize) -> &T {
        // assert!(i < self.size / ::std::mem::size_of::<T>());
        &*((self.ptr as *mut T).offset(i as isize))
    }
    pub unsafe fn get_mut<T>(&mut self, i: usize) -> &mut T {
        //assert!(i < self.size / ::std::mem::size_of::<T>());
        &mut *((self.ptr as *mut T).offset(i as isize))
    }
    pub fn new(size: usize) -> Result<Bytes, ()> {
        unsafe {
            assert!(size > 0);
            // How much memory we need:
            let half_alloc_size = no_pages(size) * page_size();
            let alloc_size = half_alloc_size.checked_mul(2).ok_or(())?;

            println!("Page size: {}", page_size());

            // 2. We allocate twice the memory, deallocate the second half, and
            // remap the first half into the second which is free for a short
            // period of time. If the remapping fails, we deallocate the first
            // half, and try again.
            let ptr = loop {
                println!("Allocating: {} bytes | total_allocation size: {}", size, alloc_size);
                // If the first allocation fails we are done:
                let ptr = alloc(alloc_size)?;
                println!("  - 1. alloc succeeded: {:?}", ptr);

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
                println!("  - 2. dealloc succeeded | ptr: {:?} | ptr_2nd_half: {:?}", ptr, ptr_2nd_half);

                // Remap the first half into the second half:
                if remap(ptr, ptr_2nd_half, half_alloc_size).is_ok() {
                    // If this succeeds, we are done:
                    println!("  - 3. remap succeeded: {:?}", ptr);
                    break ptr;
                }
                println!("  - remap failed !");

                // Otherwise, we deallocate everything and try again:
                if dealloc(ptr, half_alloc_size).is_err() {
                    // If deallocating everything also fails returning an
                    // Error would leak memory so panic:
                    panic!("failed to deallocate the 2nd half and then failed to clean up");
                }
                println!("  - dealloc succeded, retrying...");
            };
            Ok(Bytes{ ptr, size: half_alloc_size })
        }
    }
}

impl Drop for Bytes {
    fn drop(&mut self) {
        dealloc(self.ptr, self.size).unwrap()
    }
}



#[cfg(test)]
mod tests {
    use super::Bytes;

    #[test]
    fn test_alloc() {
        let mut a = Bytes::new(4096).unwrap();
        unsafe {
            for i in 0..512 {
                *a.get_mut(i) = i as u64;
            }
            for i in 512..1024 {
                assert_eq!(*a.get::<u64>(i), (i - 512) as u64);
            }
        }
    }
}

