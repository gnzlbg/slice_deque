//! Mirrored memory buffer.
mod buffer;

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android", target_os = "macos"))))]
mod sysv;
#[cfg(all(unix, not(any(target_os = "linux", target_os = "android", target_os = "macos"))))]
use self::sysv::{allocate_mirrored, allocation_granularity,
                 deallocate_mirrored};

#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;
#[cfg(any(target_os = "linux", target_os = "android"))]
use self::linux::{allocate_mirrored, allocation_granularity,
                  deallocate_mirrored};

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
use self::macos::{allocate_mirrored, allocation_granularity,
                  deallocate_mirrored};

#[cfg(target_os = "windows")]
mod winapi;

#[cfg(target_os = "windows")]
use self::winapi::{allocate_mirrored, allocation_granularity,
                   deallocate_mirrored};

pub use self::buffer::Buffer;
