//! Implements the allocator hooks on top of POSIX mmap/munmap/mremap via libc

use libc::{c_int, c_void, mmap, mremap, munmap, off_t, size_t, sysconf,
           MAP_ANONYMOUS, MAP_FAILED, MAP_NORESERVE, MAP_SHARED,
           MREMAP_FIXED, MREMAP_MAYMOVE, PROT_READ, PROT_WRITE, _SC_PAGESIZE};

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
const MAX_NO_ALLOC_ITERS: usize = 3;

/// Tries to allocates `size` bytes of memory.
///
/// # Panics
///
/// If `size` is not a multiple of the `allocation_granularity`.
fn alloc(size: usize) -> Result<*mut u8, ()> {
    assert!(size % allocation_granularity() == 0);
    assert!(size != 0);
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
            print_error("alloc");
            return Err(());
        }
        debug_assert!(!r.is_null());
        Ok(r as *mut u8)
    }
}

/// Tries to deallocates `size` bytes of memory starting at `ptr`.
///
/// # Panics
///
/// If `size` is not a multiple of the `allocation_granularity` or equals zero,
/// or if `ptr` is null.
fn dealloc(ptr: *mut u8, size: usize) -> Result<(), ()> {
    assert!(size % allocation_granularity() == 0);
    assert!(size != 0);
    assert!(!ptr.is_null());
    unsafe {
        let r: c_int = munmap(ptr as *mut c_void, size as size_t);
        if r == 0 as c_int {
            Ok(())
        } else {
            print_error("dealloc");
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
fn mirror(from: *mut u8, to: *mut u8, size: usize) -> Result<(), ()> {
    assert!(size % allocation_granularity() == 0);
    assert!(size != 0);
    assert!(!from.is_null());
    assert!(!to.is_null());
    unsafe {
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
}

/// Returns the size of a memory allocation unit.
///
/// In Linux-like systems this equal the page-size.
pub fn allocation_granularity() -> usize {
    unsafe { sysconf(_SC_PAGESIZE) as usize }
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
pub unsafe fn allocate_mirrored(alloc_size: usize) -> Result<*mut u8, ()> {
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

    Ok(ptr)
}

pub fn deallocate_mirrored(ptr: *mut u8, size: usize) {
    // On linux "linux"
    let half_alloc_size = size / 2;
    let second_half_ptr = unsafe { ptr.offset(half_alloc_size as isize) };
    // If deallocation fails while calling drop we just panic:
    dealloc(ptr, size).expect("deallocating first buffer failed")
}
