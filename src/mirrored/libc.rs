//! Implements the POSIX mmap/munmap/mremap hooks on top of libc

use libc::{c_void, size_t, c_int, off_t, _SC_PAGESIZE, sysconf, PROT_READ, PROT_WRITE,
           MAP_SHARED, MAP_NORESERVE, MAP_ANONYMOUS, MAP_FAILED, MREMAP_FIXED,MREMAP_MAYMOVE,
           mmap, munmap, mremap};

/// Allocates enough memory to store `size` bytes.
pub fn alloc(size: usize) -> Result<*mut u8, ()> {
    unsafe {
        let r: *mut c_void = mmap(
            /*addr: */ 0 as *mut c_void,
            /*length: */ size as size_t,
            /*prot: */ PROT_READ | PROT_WRITE,
            /*flags: */ MAP_SHARED | MAP_ANONYMOUS | MAP_NORESERVE,
            /*fd: */ -1 as c_int,
            /*offset: */ 0 as off_t,
        );
        if r == !0 as *mut c_void {
            return Err(());
        }
        Ok(r as *mut u8)
    }
}

/// Deallocates memory to store `size` bytes.
pub fn dealloc(ptr: *mut u8, size: usize) -> Result<(), ()> {
    unsafe {
        let r: c_int  = munmap(ptr as *mut c_void, size as size_t);
        if r == 0 as c_int {
            Ok(())
        } else {
            Err(())
        }
    }
}

pub fn remap(from: *mut u8, to: *mut u8, size: usize) -> Result<(), ()> {
    unsafe {
        let r: *mut c_void = mremap(
            /*addr: */ from as *mut c_void,
            /*len: */ 0,
            /*new_len: */ size as size_t,
            /*flags: */ MREMAP_FIXED | MREMAP_MAYMOVE,
            /*new_address: */ to as *mut c_void,
        );
        if r != MAP_FAILED && r == to as *mut c_void {
            Ok(())
        } else {
            Err(())
        }
    }
}

pub fn page_size() -> usize {
    unsafe { sysconf(_SC_PAGESIZE) as usize }
}
