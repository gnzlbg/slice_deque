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

pub fn allocation_granularity() -> usize {
    alloc_impl::allocation_granularity()
}

#[cfg(not(feature = "use_std"))]
mod cache {
    pub fn allocate_mirrored(size: usize) -> Result<*mut u8, ()> {
        super::alloc_impl::allocate_mirrored(size)
    }
    pub unsafe fn deallocate_mirrored(ptr: *mut u8, size: usize) {
        super::alloc_impl::deallocate_mirrored(ptr, size);
    }
    pub fn release() {}
}

#[cfg(feature = "use_std")]
mod cache {
    use ::std::sync::Mutex;
    use ::std::collections::HashMap;
    lazy_static! {
        static ref POOL: Mutex<HashMap<usize, Vec<usize>>>
            = Mutex::new(HashMap::new());
    }

    pub fn allocate_mirrored(size: usize) -> Result<*mut u8, ()> {
        let mut pool = POOL.lock().unwrap();
        let mut remove_entry = false;
        let r = match pool.get_mut(&size) {
            Some(ref mut sizes) => {
                debug_assert!(!sizes.is_empty());
                let ptr = sizes.pop().unwrap();
                remove_entry = sizes.is_empty();
                Ok(ptr as *mut u8)
            },
            None => super::alloc_impl::allocate_mirrored(size),
        };
        if remove_entry {
            let o = pool.remove(&size);
            debug_assert!(o.is_some() && o.unwrap().is_empty());
        }
        r
    }

    pub unsafe fn deallocate_mirrored(ptr: *mut u8, size: usize) {
        let mut pool = POOL.lock().unwrap();
        pool.entry(size).and_modify(|v| {
            v.push(ptr as usize);
        }).or_insert_with(|| {
            let mut v = Vec::with_capacity(1);
            v.push(ptr as usize);
            v
        });
    }

    pub fn release() {
        unsafe {
            let mut pool = POOL.lock().unwrap();
            for (&size, vec) in pool.iter() {
                for &ptr in vec {
                    super::alloc_impl::deallocate_mirrored(ptr as *mut u8, size);
                }
            }
            pool.clear();
        }
    }
}

pub use self::cache::*;
