//! Implements the allocator hooks on top of window's virtual alloc.

use winapi::shared::basetsd::SIZE_T;
use winapi::shared::ntdef::LPCWSTR;
use winapi::shared::minwindef::{BOOL, DWORD, LPCVOID, LPVOID};
use winapi::um::memoryapi::{CreateFileMappingW, MapViewOfFileEx,
                            UnmapViewOfFile, VirtualAlloc, VirtualFree,
                            FILE_MAP_ALL_ACCESS};
use winapi::um::winnt::{MEM_RELEASE, MEM_RESERVE, PAGE_NOACCESS,
                        PAGE_READWRITE, SEC_COMMIT};

use winapi::um::minwinbase::LPSECURITY_ATTRIBUTES;
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
use winapi::um::sysinfoapi::{GetSystemInfo, LPSYSTEM_INFO, SYSTEM_INFO};

pub use winapi::shared::ntdef::HANDLE;

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
const MAX_NO_ALLOC_ITERS: usize = 5;

/// Creates a file mapping able to hold `alloc_size` bytes.
///
/// # Panics
///
/// If `size` is not a multiple of the `allocation_granularity`.
fn create_file_mapping(size: usize) -> Result<HANDLE, ()> {
    assert!(size % allocation_granularity() == 0);
    let dw_maximum_size_low: DWORD = size as DWORD;
    let dw_maximum_size_high: DWORD = match (
        ::std::mem::size_of::<DWORD>(),
        ::std::mem::size_of::<usize>(),
    ) {
        // If both sizes are equal, the size is passed in the lower half,
        // so the higher 32-bits are zero
        (4, 4) | (8, 8) => 0,
        // If DWORD is 32 bit but usize is 64-bit, we pass the higher 32-bit of
        // size:
        (4, 8) => (size >> 32) as DWORD,
        _ => unimplemented!(),
    };
    unsafe {
        let h: HANDLE = CreateFileMappingW(
            /* hFile: */ INVALID_HANDLE_VALUE as HANDLE,
            /* lpAttributes: */ 0 as LPSECURITY_ATTRIBUTES,
            /* flProtect: */ PAGE_READWRITE | SEC_COMMIT as DWORD,
            /* dwMaximumSizeHigh: */ dw_maximum_size_high,
            /* dwMaximumSizeLow: */ dw_maximum_size_low,
            /* lpName: */ 0 as LPCWSTR,
        );

        if h.is_null() {
            print_error("create_file_mapping");
            #[cfg(build = "debug")]
            eprintln!(
                "failed to create a file mapping with size {} bytes",
                size
            );
            return Err(());
        }
        Ok(h)
    }
}

/// Closes a file mapping.
///
/// # Panics
///
/// If `file_mapping` is null.
fn close_file_mapping(file_mapping: HANDLE) -> Result<(), ()> {
    assert!(!file_mapping.is_null());
    unsafe {
        let r: BOOL = CloseHandle(file_mapping);
        if r == 0 {
            print_error("close_file_mapping");
            return Err(());
        }
        Ok(())
    }
}

/// Reserves a virtual memory region able to hold `size` bytes.
///
/// The Windows API has no way to do this, so... we allocate a `size`-ed region
/// with `VirtualAlloc`, immediately free it afterwards with `VirtualFree` and
/// hope that the region is still available when we try to map into it.
///
/// # Panics
///
/// If `size` is not a multiple of the `allocation_granularity`.
fn reserve_virtual_memory(size: usize) -> Result<(*mut u8), ()> {
    assert!(size % allocation_granularity() == 0);
    unsafe {
        let r: LPVOID = VirtualAlloc(
            /* lpAddress: */ 0 as LPVOID,
            /* dwSize: */ size as SIZE_T,
            /* flAllocationType: */ MEM_RESERVE,
            /* flProtect: */ PAGE_NOACCESS,
        );

        if r.is_null() {
            print_error("reserve_virtual_memory(alloc failed)");
            return Err(());
        }

        let fr = VirtualFree(
            /* lpAddress: */ r,
            /* dwSize: */ 0 as SIZE_T,
            /* dwFreeType: */ MEM_RELEASE as DWORD,
        );
        if fr == 0 {
            print_error("reserve_virtual_memory(free failed)");
            return Err(());
        }

        Ok(r as *mut u8)
    }
}

/// Maps `size` bytes of `file_mapping` to `adress`.
///
/// # Panics
///
/// If `file_mapping` or `address` are null, or if `size` is not a multiple of
/// the allocation granularity of the system.
fn map_file_to_memory(
    file_mapping: HANDLE, size: usize, address: *mut u8
) -> Result<(), ()> {
    assert!(!file_mapping.is_null());
    assert!(!address.is_null());
    assert!(size % allocation_granularity() == 0);
    unsafe {
        let r: LPVOID = MapViewOfFileEx(
            /* hFileMappingObject: */ file_mapping,
            /* dwDesiredAccess: */ FILE_MAP_ALL_ACCESS,
            /* dwFileOffsetHigh: */ 0 as DWORD,
            /* dwFileOffsetLow: */ 0 as DWORD,
            /* dwNumberOfBytesToMap: */ size as SIZE_T,
            /* lpBaseAddress: */ address as LPVOID,
        );
        if r.is_null() {
            print_error("map_file_to_memory");
            return Err(());
        }
        debug_assert!(r == address as LPVOID);
        Ok(())
    }
}

/// Unmaps the memory at `address`.
///
/// # Panics
///
/// If `address` is null.
fn unmap_file_from_memory(address: *mut u8) -> Result<(), ()> {
    assert!(!address.is_null());
    unsafe {
        let r = UnmapViewOfFile(/* lpBaseAddress: */ address as LPCVOID);
        if r == 0 {
            print_error("unmap_file_from_memory");
            return Err(());
        }
        Ok(())
    }
}

/// Returns the size of an allocation unit in bytes.
///
/// In Windows calls to `VirtualAlloc` must specify a multiple of
/// `SYSTEM_INFO::dwAllocationGranularity` bytes.
///
/// FIXME: the allocation granularity should always be larger than the page
/// size (64k vs 4k), so determining the page size here is not necessary.
pub fn allocation_granularity() -> usize {
    unsafe {
        let mut system_info: SYSTEM_INFO = ::std::mem::uninitialized();
        GetSystemInfo(&mut system_info as LPSYSTEM_INFO);
        let allocation_granularity =
            system_info.dwAllocationGranularity as usize;
        let page_size = system_info.dwPageSize as usize;
        page_size.max(allocation_granularity)
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
pub unsafe fn allocate_mirrored(alloc_size: usize) -> Result<*mut u8, ()> {
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
        if map_file_to_memory(file_mapping, half_alloc_size, virt_ptr).is_err()
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
    close_file_mapping(file_mapping).expect("closing file handle failed");

    Ok(virt_ptr)
}

pub fn deallocate_mirrored(ptr: *mut u8, size: usize) {
    // On "windows" we unmap the memory.
    let half_alloc_size = size / 2;
    unmap_file_from_memory(ptr).expect("unmapping first buffer half failed");
    let second_half_ptr = unsafe { ptr.offset(half_alloc_size as isize) };
    unmap_file_from_memory(second_half_ptr)
        .expect("unmapping second buffer half failed");
}
