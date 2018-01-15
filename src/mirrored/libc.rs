//! Implements the allocator hooks on top of POSIX mmap/munmap/mremap via libc

use libc::{c_int, c_void, mmap, mremap, munmap, off_t, size_t, sysconf,
           MAP_ANONYMOUS, MAP_FAILED, MAP_NORESERVE, MAP_SHARED,
           MREMAP_FIXED, MREMAP_MAYMOVE, PROT_READ, PROT_WRITE, _SC_PAGESIZE};

/// Tries to allocates `size` bytes of memory.
///
/// # Panics
///
/// If `size` is not a multiple of the `allocation_granularity`.
pub fn alloc(size: usize) -> Result<*mut u8, ()> {
    assert!(size % allocation_granularity() == 0);
    unsafe {
        let r: *mut c_void = mmap(
            /* addr: */ 0 as *mut c_void,
            /* length: */ size as size_t,
            /* prot: */ PROT_READ | PROT_WRITE,
            /* flags: */ MAP_SHARED | MAP_ANONYMOUS | MAP_NORESERVE,
            /* fd: */ -1 as c_int,
            /* offset: */ 0 as off_t,
        );
        if r == !0 as *mut c_void {
            return Err(());
        }
        Ok(r as *mut u8)
    }
}

/// Tries to deallocates `size` bytes of memory starting at `ptr`.
///
/// # Panics
///
/// If `size` is not a multiple of the `allocation_granularity`.
pub fn dealloc(ptr: *mut u8, size: usize) -> Result<(), ()> {
    assert!(size % allocation_granularity() == 0);
    unsafe {
        let r: c_int = munmap(ptr as *mut c_void, size as size_t);
        if r == 0 as c_int {
            Ok(())
        } else {
            Err(())
        }
    }
}

/// Mirrors `size` bytes of memory starting at `from` to a memory region
/// starting at `to`.
///
/// # Panics
///
/// If `size` is not a multiple of the `allocation_granularity`.
pub fn mirror(from: *mut u8, to: *mut u8, size: usize) -> Result<(), ()> {
    assert!(size % allocation_granularity() == 0);
    unsafe {
        let r: *mut c_void = mremap(
            /* addr: */ from as *mut c_void,
            /* len: */ 0,
            /* new_len: */ size as size_t,
            /* flags: */ MREMAP_FIXED | MREMAP_MAYMOVE,
            /* new_address: */ to as *mut c_void,
        );
        if r != MAP_FAILED && r == to as *mut c_void {
            Ok(())
        } else {
            Err(())
        }
    }
}

/// Returns the size of a memory allocation unit.
///
/// In Linux-like systems this equal the page-size.
pub fn allocation_granularity() -> usize {
    unsafe { sysconf(_SC_PAGESIZE) as usize }
}
