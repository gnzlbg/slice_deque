//! Implements the allocator hooks on top of window's virtual alloc.

use kernel32::{VirtualAlloc, LPVOID, SIZE_T, DWORD,
               MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE,
               // TODO: NULL ?
               VirtualFree, MEM_RELEASE, BOOL
};

/// Creates a file mapping able to hold `alloc_size` bytes.
///
/// # Panics
///
/// If `size` is not a multiple of the `page_size`.
pub fn create_file_mapping(size: usize) -> Result<HANDLE, ()> {
    assert!(size % page_size() == 0);
    unsafe {
        let h: HANDLE = CreateFileMappingA(
            /* hFile: */ INVALID_HANDLE_VALUE as HANDLE,
            /* lpAttributes: */ NULL as LPSECURITY_ATTRIBUTES,
            /* flProtect: */ PAGE_READWRITE | SEC_COMMIT as DWORD,
            /* dwMaximumSizeHigh: */ (alloc_size as DWORD) >> 32,
            /* dwMaximumSizeLow: */ (alloc_size as DWORD) & 0xFFFFFFFFU,
            /* lpName: */ NULL as LPCSTR);

        if r == NULL /*0 as *mut LPVOID */ {
            return Err(());
        }
        Ok(r)
    }
}

/// Closes a file mapping.
pub fn close_file_mapping(file_mapping: HANDLE) -> Result<(), ()> {
    let r: BOOL = CloseHandle(file_mapping);
    if r == NULL {
        return Err(());
    }
    Ok(())
}

/// Reserves a virtual memory region able to hold `size` bytes.
///
/// The Windows API has no way to do this, so... we allocate a `size`-ed region
/// with `VirtualAlloc`, immediately free it afterwards with `VirtualFree` and
/// hope that the region is still available when we try to map into it.
///
/// # Panics
///
/// If `size` is not a multiple of the `page_size`.
pub fn reserve_virtual_memory(size: usize) -> Result<(*mut u8), ()> {
    assert!(size % page_size() == 0);

    let r: *mut LPVOID = VirtualAlloc(
        /*lpAddress:*/ NULL as LPVOID,
        /*dwSize:*/ size as SIZE_T,
        /*flAllocationType:*/ MEM_RESERVE,
        /*flProtect:*/ PAGE_NOACCESS
    );

    if r == NULL {
        return Err(());
    }

    let fr = VirtualFree(
        /*lpAddress: */r as *mut LPVOID,
        /*dwSize: */0 as SIZE_T,
        /*dwFreeType: */ MEM_RELEASE as DWORD);
    if fr == NULL {
        return Err(());
    }

    Ok(r as *mut u8)
}

/// Maps `size` bytes of `file_mapping` to `adress`.
pub fn map_file_to_memory(file_mapping: HANDLE, size: usize, address: *mut u8)
                          -> Result<(), ()> {
    let r: LPVOID = MapViewOfFileEx(
        /*hFileMappingObject: */ file_mapping,
        /*dwDesiredAccess: */ FILE_MAP_ALL_ACCESS,
        /*dwFileOffsetHigh: */ 0 as DWORD,
        /*dwFileOffsetLow: */ 0 as DWORD,
        /*dwNumberOfBytesToMap: */ size as SIZE_T,
        /*lpBaseAddress: */ address as LPVOID
    );
    if r == NULL {
        return Err(());
    }
    debug_assert!(r == address as LPVOID);
    Ok(())
}

/// Unmaps the memory at `address`.
pub fn unmap_file_mapped_memory(address: *mut u8) -> Result<(), ()> {
    let r = UnmapViewOfFile(/*lpBaseAddress: */ address as LPCVOID);
    if r == 0 as BOOL {
        return Err(());
    }
    Ok(())
}

/// Returns the size of an allocation unit in bytes.
///
/// In Windows calls to `VirtualAlloc` must specify a multiple of
/// `SYSTEM_INFO::dwAllocationGranularity` bytes.
///
/// FIXME: the allocation granularity should always be larger than the page size
/// (64k vs 4k), so determining the page size here is not necessary.
pub fn allocation_granularity() -> usize {
    unsafe {
        let page_size = sysconf(_SC_PAGESIZE) as usize;
        let mut system_info: SYSTEM_INFO = ::std::mem::uninitialized();
        GetSystemInfo(&mut system_info as LPSYSTEM_INFO);
        let allocation_granularity = system_info.dwAllocationGranularity as usize;
        page_size.max(allocation_granularity)
    }
}
