//! Mirrored memory buffer.
mod buffer;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
use self::macos::*;

#[cfg(target_os = "linux")]
mod libc;

#[cfg(target_os = "linux")]
use self::libc::*;

#[cfg(target_os = "windows")]
mod winapi;

#[cfg(target_os = "windows")]
use self::winapi::*;

pub use self::buffer::Buffer;

#[cfg(target_os = "windows")]
pub use self::winapi::HANDLE;
