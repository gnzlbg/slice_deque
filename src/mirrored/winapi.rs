//! Implements the allocator hooks on top of window's virtual alloc.

use winapi::shared::basetsd::SIZE_T;
use winapi::shared::ntdef::LPCWSTR;
use winapi::shared::minwindef::{BOOL, DWORD, LPCVOID, LPVOID};
use winapi::um::memoryapi::{MapViewOfFileEx, UnmapViewOfFile, VirtualAlloc,
                            VirtualFree, FILE_MAP_ALL_ACCESS, CreateFileMappingW};
use winapi::um::winnt::{MEM_RELEASE, MEM_RESERVE, PAGE_NOACCESS,
                        PAGE_READWRITE, SEC_COMMIT};

use winapi::um::minwinbase::LPSECURITY_ATTRIBUTES;
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
use winapi::um::sysinfoapi::{GetSystemInfo, LPSYSTEM_INFO, SYSTEM_INFO};

pub use winapi::shared::ntdef::HANDLE;

/// Creates a file mapping able to hold `alloc_size` bytes.
///
/// # Panics
///
/// If `size` is not a multiple of the `allocation_granularity`.
pub fn create_file_mapping(size: usize) -> Result<HANDLE, ()> {
    assert!(size % allocation_granularity() == 0);
    let dw_maximum_size_high: DWORD = match (
        ::std::mem::size_of::<DWORD>(),
        ::std::mem::size_of::<usize>(),
    ) {
        // If both sizes are equal, the size is passed in the lower half
        (4, 4) | (8, 8) => 0,
        (4, 8) => (size >> 32) as DWORD,
        _ => unimplemented!(),
    };
    let dw_maximum_size_low: DWORD = match (
        ::std::mem::size_of::<DWORD>(),
        ::std::mem::size_of::<usize>(),
    ) {
        (4, 4) | (8, 8) => size as DWORD,
        (4, 8) => (size & 0xFFFF_FFFF) as DWORD,
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
            eprintln!("failed to create a file mapping with size {}", size);
            print_error("create_file_mapping");
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
pub fn close_file_mapping(file_mapping: HANDLE) -> Result<(), ()> {
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
pub fn reserve_virtual_memory(size: usize) -> Result<(*mut u8), ()> {
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
pub fn map_file_to_memory(
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
pub fn unmap_file_from_memory(address: *mut u8) -> Result<(), ()> {
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


fn print_error(location: &str) {
    eprintln!("Error at {}: {}", location, ::std::io::Error::last_os_error());
}
