//! Implements the allocator hooks on top of mach.

use mach;
use mach::kern_return::{kern_return_t, KERN_SUCCESS};
use mach::vm_types::mach_vm_address_t;
use mach::vm_prot::vm_prot_t;
use mach::vm::{mach_vm_allocate, mach_vm_deallocate, mach_vm_remap};
use mach::traps::mach_task_self;
use mach::vm_statistics::VM_FLAGS_ANYWHERE;
use mach::vm_inherit::VM_INHERIT_COPY;

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
/// # Panics
///
/// If `size` is not a multiple of the `allocation_granularity`.
fn dealloc(ptr: *mut u8, size: usize) -> Result<(), ()> {
    assert!(size % allocation_granularity() == 0);
    unsafe {
        let addr = ptr as mach_vm_address_t;
        let r: kern_return_t =
            mach_vm_deallocate(mach_task_self(), addr, size as u64);
        if r == KERN_SUCCESS {
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
fn mirror(from: *mut u8, to: *mut u8, size: usize) -> Result<(), ()> {
    assert!(size % allocation_granularity() == 0);
    unsafe {
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
}

/// Returns the size of an allocation unit.
///
/// In `MacOSX` this equals the page size.
pub fn allocation_granularity() -> usize {
    unsafe { mach::vm_page_size::vm_page_size as usize }
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

// On "macos" we can deallocate the non-mirrored and mirrored parts of
// the buffer at once:
// If deallocation fails while calling drop we just panic:
pub fn deallocate_mirrored(ptr: *mut u8, size: usize) {
    dealloc(ptr, size).expect("deallocating mirrored buffer failed");
}
