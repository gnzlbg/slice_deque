//! Implements the allocator hooks on top of mach.

use mach;
use mach::kern_return::{kern_return_t, KERN_SUCCESS};
use mach::vm_types::mach_vm_address_t;
use mach::vm_prot::vm_prot_t;
use mach::vm::{mach_vm_allocate, mach_vm_deallocate, mach_vm_remap};
use mach::traps::mach_task_self;
use mach::vm_statistics::VM_FLAGS_ANYWHERE;
use mach::vm_inherit::VM_INHERIT_COPY;

pub fn alloc(size: usize) -> Result<*mut u8, ()> {
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

pub fn dealloc(ptr: *mut u8, size: usize) -> Result<(), ()> {
    unsafe {
        let addr = ptr as mach_vm_address_t;
        let r: kern_return_t = mach_vm_deallocate(mach_task_self(), addr, size as u64);
        if r == KERN_SUCCESS {
            Ok(())
        } else {
            Err(())
        }
    }
}

pub fn remap(from: *mut u8, to: *mut u8, size: usize) -> Result<(), ()> {
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

pub fn page_size() -> usize {
    unsafe { mach::vm_page_size::vm_page_size as usize }
}
