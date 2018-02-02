//! Mirrored memory buffer.
mod buffer;

#[cfg(all(unix,
          not(any(all(target_os = "linux", not(feature = "unix_sysv")),
                  all(target_os = "android",
                      not(feature = "unix_sysv")),
                  // FIXME: libc does not support MacOSX shared memory yet
                  target_os = "macos"
          ))))]
mod sysv;
#[cfg(all(unix,
          not(any(all(target_os = "linux", not(feature = "unix_sysv")),
                  all(target_os = "android",
                      not(feature = "unix_sysv")),
                  // FIXME: libc does not support MacOSX shared memory yet
                  target_os = "macos"
          ))))]
use self::sysv::{allocate_mirrored, allocation_granularity,
                 deallocate_mirrored};

#[cfg(all(any(target_os = "linux", target_os = "android"),
          not(feature = "unix_sysv")))]
mod linux;
#[cfg(all(any(target_os = "linux", target_os = "android"),
          not(feature = "unix_sysv")))]
use self::linux::{allocate_mirrored, allocation_granularity,
                  deallocate_mirrored};

// FIXME: libc does not support MacOSX shared memory yet
#[cfg(target_os = "macos")]
mod macos;

// FIXME: libc does not support MacOSX shared memory yet
#[cfg(target_os = "macos")]
use self::macos::{allocate_mirrored, allocation_granularity,
                  deallocate_mirrored};

#[cfg(target_os = "windows")]
mod winapi;

#[cfg(target_os = "windows")]
use self::winapi::{allocate_mirrored, allocation_granularity,
                   deallocate_mirrored};

pub use self::buffer::Buffer;

use super::*;
