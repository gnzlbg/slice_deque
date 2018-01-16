//! Implements the allocator hooks on top of POSIX mmap/munmap/mremap via libc

use libc::{c_int, c_void, mmap, mremap, munmap, off_t, size_t, sysconf,
           MAP_ANONYMOUS, MAP_FAILED, MAP_NORESERVE, MAP_SHARED,
           MREMAP_FIXED, MREMAP_MAYMOVE, PROT_READ, PROT_WRITE, _SC_PAGESIZE};

/// Returns the size of a memory allocation unit.
///
/// In Linux-like systems this equals the page-size.
pub fn allocation_granularity() -> usize {
    unsafe { sysconf(_SC_PAGESIZE) as usize }
}

/// Allocates an uninitialzied buffer that holds `size` bytes, where
/// the bytes in range `[0, size / 2)` are mirrored into the bytes in
/// range `[size / 2, size)`.
///
/// On Linux the algorithm is as follows:
///
/// * 1. Allocate twice the memory (`size` bytes)
/// * 2. Deallocate the second half (bytes in range `[size / 2, 0)`)
/// * 3. Race condition: mirror bytes of the first half into the second
/// half.
///
/// If we get a race (e.g. because some other process allocates to the
/// second half) we release all the resources (we need to deallocate the
/// memory) and try again (up to a maximum of `MAX_NO_ALLOC_ITERS` times).
///
/// # Panics
///
/// If `size` is zero or `size / 2` is not a multiple of the
/// allocation granularity.
pub fn allocate_mirrored(size: usize) -> Result<*mut u8, ()> {
    unsafe {
        let half_size = size / 2;
        assert!(size != 0);
        assert!(half_size % allocation_granularity() == 0);

        // Maximum number of allocation iterations.
        const MAX_NO_ALLOC_ITERS: usize = 3;

        let mut no_iters = 0;
        let ptr = loop {
            if no_iters > MAX_NO_ALLOC_ITERS {
                panic!("number of iterations exceeded!");
            }

            // If the first allocation fails we are done:
            let ptr = alloc(size)?;

            // This cannot overflow isize: the worst case size is
            // usize::MAX, where size / 2 == isize::MAX.
            let ptr_2nd_half = ptr.offset(half_size as isize);
            dealloc(ptr_2nd_half, half_size).map_err(|()| {
                // If deallocating the second half fails we deallocate
                // everything and fail:
                if dealloc(ptr, size).is_err() {
                    // If deallocating everything also fails returning an
                    // Error would leak memory so panic:
                    panic!("failed to deallocate the 2nd half and then failed to clean up");
                }
                ()
            })?;

            // Mirror the first half into the second half:
            if mirror(ptr, ptr_2nd_half, half_size).is_ok() {
                // If this succeeds, we are done:
                break ptr;
            }

            // Otherwise, we deallocate everything and try again:
            if dealloc(ptr, half_size).is_err() {
                // If deallocating everything also fails returning an
                // Error would leak memory so panic:
                panic!("failed to deallocate the 2nd half and then failed to clean up");
            }
            no_iters += 1;
        };

        Ok(ptr)
    }
}

/// Deallocates the mirrored memory region at `ptr` of `size` bytes.
///
/// # Unsafe
///
/// `ptr` must have been obtained from a call to `allocate_mirrored(size)`,
/// otherwise the behavior is undefined.
///
/// # Panics
///
/// If `size` is zero or `size / 2` is not a multiple of the
/// allocation granularity, or `ptr` is null.
pub unsafe fn deallocate_mirrored(ptr: *mut u8, size: usize) {
    assert!(!ptr.is_null());
    assert!(size != 0);
    assert!(size % allocation_granularity() == 0);
    dealloc(ptr, size).expect("deallocating buffer failed")
}

/// Tries to allocates `size` bytes of memory.
///
/// # Panics
///
/// If `size` is zero or not a multiple of the `allocation_granularity`.
fn alloc(size: usize) -> Result<*mut u8, ()> {
    unsafe {
        assert!(size % allocation_granularity() == 0);
        assert!(size != 0);

        let r: *mut c_void = mmap(
            /* addr: */ 0 as *mut c_void,
            /* length: */ size as size_t,
            /* prot: */ PROT_READ | PROT_WRITE,
            /* flags: */ MAP_SHARED | MAP_ANONYMOUS | MAP_NORESERVE,
            /* fd: */ -1 as c_int,
            /* offset: */ 0 as off_t,
        );
        if r == !0 as *mut c_void {
            print_error("alloc");
            return Err(());
        }
        debug_assert!(!r.is_null());
        Ok(r as *mut u8)
    }
}

/// Tries to deallocates `size` bytes of memory starting at `ptr`.
///
/// # Unsafety
///
/// The `ptr` must have been obtained from a previous call to `alloc(size)`.
///
/// # Panics
///
/// If `size` is zero or not a multiple of the `allocation_granularity`, or if
/// `ptr` is null.
unsafe fn dealloc(ptr: *mut u8, size: usize) -> Result<(), ()> {
    assert!(size % allocation_granularity() == 0);
    assert!(size != 0);
    assert!(!ptr.is_null());
    let r: c_int = munmap(ptr as *mut c_void, size as size_t);
    if r == 0 as c_int {
        Ok(())
    } else {
        print_error("dealloc");
        Err(())
    }
}

/// Mirrors `size` bytes of memory starting at `from` to a memory region
/// starting at `to`.
///
/// # Unsafety
///
/// The `from` pointer must have been obtained from a previous call to `alloc`
/// and point to a memory region containing at least `size` bytes.
///
/// # Panics
///
/// If `size` zero or ot a multiple of the `allocation_granularity`, or if
/// `from` or `to` are null.
unsafe fn mirror(from: *mut u8, to: *mut u8, size: usize) -> Result<(), ()> {
    assert!(size % allocation_granularity() == 0);
    assert!(size != 0);
    assert!(!from.is_null());
    assert!(!to.is_null());
    let r: *mut c_void = mremap(
        /* old_addr: */ from as *mut c_void,
        /* old_size: */ 0,
        /* new_size: */ size as size_t,
        /* flags: */ MREMAP_FIXED | MREMAP_MAYMOVE,
        /* new_address: */ to as *mut c_void,
    );
    if r != MAP_FAILED && r == to as *mut c_void {
        Ok(())
    } else {
        print_error("mirror");
        Err(())
    }
}

#[cfg(debug_assertions)]
fn print_error(location: &str) {
    eprintln!(
        "Error at {}: {}",
        location,
        ::std::io::Error::last_os_error()
    );
}

#[cfg(not(debug_assertions))]
fn print_error(_location: &str) {}
