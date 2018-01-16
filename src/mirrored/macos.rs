//! Implements the allocator hooks on top of mach.

use mach;
use mach::kern_return::{kern_return_t, KERN_SUCCESS};
use mach::vm_types::mach_vm_address_t;
use mach::vm_prot::vm_prot_t;
use mach::vm::{mach_vm_allocate, mach_vm_deallocate, mach_vm_remap};
use mach::traps::mach_task_self;
use mach::vm_statistics::VM_FLAGS_ANYWHERE;
use mach::vm_inherit::VM_INHERIT_COPY;

/// Returns the size of an allocation unit.
///
/// In `MacOSX` this equals the page size.
pub fn allocation_granularity() -> usize {
    unsafe { mach::vm_page_size::vm_page_size as usize }
}

/// Allocates an uninitialzied buffer that holds `size` bytes, where
/// the bytes in range `[0, size / 2)` are mirrored into the bytes in
/// range `[size / 2, size)`.
///
/// On Macos X the algorithm is as follows:
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
    dealloc(ptr, size).expect("deallocating mirrored buffer failed");
}

/// Tries to allocates `size` bytes of memory.
///
/// # Panics
///
/// If `size` is zero or not a multiple of the `allocation_granularity`.
fn alloc(size: usize) -> Result<*mut u8, ()> {
    assert!(size != 0);
    assert!(size % allocation_granularity() == 0);
    unsafe {
        let mut addr: mach_vm_address_t = 0;
        let r: kern_return_t = mach_vm_allocate(
            mach_task_self(),
            &mut addr as *mut mach_vm_address_t,
            size as u64,
            VM_FLAGS_ANYWHERE,
        );
        if r != KERN_SUCCESS {
            // If the first allocation fails, there is nothing to
            // deallocate and we can just fail to allocate:
            return Err(());
        }
        Ok(addr as *mut u8)
    }
}

/// Tries to deallocates `size` bytes of memory starting at `ptr`.
///
/// # Unsafety
///
/// The `ptr` must have been obtained from a previous call to `alloc` and point
/// to a memory region containing at least `size` bytes.
///
/// # Panics
///
/// If `size` is zero or not a multiple of the `allocation_granularity`, or if
/// `ptr` is null.
unsafe fn dealloc(ptr: *mut u8, size: usize) -> Result<(), ()> {
    assert!(size != 0);
    assert!(size % allocation_granularity() == 0);
    assert!(!ptr.is_null());
    let addr = ptr as mach_vm_address_t;
    let r: kern_return_t =
        mach_vm_deallocate(mach_task_self(), addr, size as u64);
    if r == KERN_SUCCESS {
        Ok(())
    } else {
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
    assert!(!from.is_null());
    assert!(!to.is_null());
    assert!(size != 0);
    assert!(size % allocation_granularity() == 0);
    let mut cur_protection: vm_prot_t = 0;
    let mut max_protection: vm_prot_t = 0;
    let mut to = to as mach_vm_address_t;
    let r: kern_return_t = mach_vm_remap(
        mach_task_self(),
        &mut to,
        size as u64,
        /* mask: */ 0,
        /* anywhere: */ 0,
        mach_task_self(),
        from as u64,
        /* copy */ 0,
        &mut cur_protection,
        &mut max_protection,
        VM_INHERIT_COPY,
    );
    if r == KERN_SUCCESS {
        Ok(())
    } else {
        Err(())
    }
}
