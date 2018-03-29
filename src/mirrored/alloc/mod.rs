//! Mirrored memory allocator.

/// SystemV IPC implementation.
#[cfg(all(unix,
          not(all(any(target_os = "linux", target_os = "android",
                      target_os = "macos", target_os = "ios"),
                  not(feature = "unix_sysv")))))]
#[path = "sysv.rs"]
mod alloc_impl;

/// Linux-specific implementation.
#[cfg(all(any(target_os = "linux", target_os = "android"),
          not(feature = "unix_sysv")))]
#[path = "linux.rs"]
mod alloc_impl;

/// Mach-specific implementation.
#[cfg(all(any(target_os = "macos", target_os = "ios"),
          not(feature = "unix_sysv")))]
#[path = "macos.rs"]
mod alloc_impl;

/// WinAPI implementation.
#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod alloc_impl;

/// Smallest allocation size of the Operating System.
pub fn allocation_granularity() -> usize {
    alloc_impl::allocation_granularity()
}

#[cfg(not(feature = "use_std"))]
mod cache {
    /// Allocates `size` bytes.
    pub fn allocate_mirrored(size: usize) -> Result<*mut u8, ()> {
        super::alloc_impl::allocate_mirrored(size)
    }
    /// Deallocates the memory in range `[ptr, ptr + size)`
    pub unsafe fn deallocate_mirrored(ptr: *mut u8, size: usize) {
        super::alloc_impl::deallocate_mirrored(ptr, size);
    }
    /// Frees all memory reserved by this thread.
    pub fn release() {}
}

/// Caches previously-freed allocations for reuse.
#[cfg(feature = "use_std")]
mod cache {
    use super::allocation_granularity;
    thread_local! { static POOL: ::std::cell::UnsafeCell<Cache> = ::std::cell::UnsafeCell::new(Cache::new()); }

    /// Previously-freed allocation cache.
    struct Cache {
        /// Caches the smallest possible allocations of size `2 *
        /// allocation_granularity()` bytes.
        smallest: Vec<usize>,
        /// Caches all other allocations.
        other: ::std::collections::HashMap<usize, Vec<usize>>
    }

    impl Cache {
        /// Instantiates a new empty cache.
        pub fn new() -> Self { Self { smallest: Vec::with_capacity(512), other: ::std::collections::HashMap::new() } }
        /// Tries to fetch an allocation of size `size` from the cache.
        pub fn pop(&mut self, size: usize) -> Option<*mut u8> {
            if size == 2 * allocation_granularity() {
                return self.smallest.pop().map(|v| v as *mut u8);
            }
            if let Some(ref mut sizes) = self.other.get_mut(&size) {
                if let Some(v) = sizes.pop() {
                    return Some(v as *mut u8);
                }
            }
            None
        }
        /// Caches a memory region of size `[ptr, ptr + size)` for future reuse.
        pub fn push(&mut self, ptr: *mut u8, size: usize) -> bool {
            if size == 2 * allocation_granularity() {
                self.smallest.push(ptr as usize);
                return true;
            }
            self.other.entry(size).and_modify(|v| {
                v.push(ptr as usize);
            }).or_insert_with(|| {
                let mut v = Vec::with_capacity(128);
                v.push(ptr as usize);
                v
            });
            true
        }
    }

    impl Drop for Cache {
        fn drop(&mut self) {
            for &ptr in &self.smallest {
                unsafe { super::alloc_impl::deallocate_mirrored(ptr as *mut u8, 2 * allocation_granularity()) }; 
            }
            for (&size, vec) in &self.other {
                for &ptr in vec {
                    unsafe { super::alloc_impl::deallocate_mirrored(ptr as *mut u8, size) };
                }
            }
        }
    }
    /// Allocates `size` bytes.
    pub fn allocate_mirrored(size: usize) -> Result<*mut u8, ()> {
        if let Some(v) = POOL.with(|v| (unsafe { &mut *v.get() }).pop(size)) {
            return Ok(v);
        }
        super::alloc_impl::allocate_mirrored(size)
    }

    /// Deallocates the memory in range `[ptr, ptr + size)`
    pub unsafe fn deallocate_mirrored(ptr: *mut u8, size: usize) {
        if !POOL.with(|v| (&mut *v.get()).push(ptr, size) ) {
            super::alloc_impl::deallocate_mirrored(ptr, size);
        }
    }

    /// Frees all memory reserved by this thread.
    pub fn release() {
        POOL.with(|v| {
            let mut c = Cache::new();
            ::std::mem::swap(&mut c, unsafe { &mut *v.get() });
        })
    }
}

pub use self::cache::*;
