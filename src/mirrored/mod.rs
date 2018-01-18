//! Mirrored memory buffer.
mod buffer;

#[cfg(all(unix, not(target_os = "linux")))]
mod sysv;
#[cfg(all(unix, not(target_os = "linux")))]
use self::sysv::{allocate_mirrored, allocation_granularity,
                 deallocate_mirrored};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
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
