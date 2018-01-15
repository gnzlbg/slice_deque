//! Mirrored memory buffer.
mod buffer;

#[cfg(target_os = "macos")]
mod darwin;

#[cfg(target_os = "macos")]
use self::darwin::*;

#[cfg(target_os = "linux")]
mod libc;

#[cfg(target_os = "linux")]
use self::libc::*;

#[cfg(target_os = "windows")]
mod kernel32;

#[cfg(target_os = "windows")]
use self::kernel32::*;

pub use self::buffer::Buffer;
