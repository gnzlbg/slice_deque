//! Mirrored memory buffer.
mod buffer;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
use self::macos::{allocate_mirrored, deallocate_mirrored, allocation_granularity};

#[cfg(target_os = "linux")]
mod libc;

#[cfg(target_os = "linux")]
use self::libc::{allocate_mirrored, deallocate_mirrored, allocation_granularity};

#[cfg(target_os = "windows")]
mod winapi;

#[cfg(target_os = "windows")]
use self::winapi::{allocate_mirrored, deallocate_mirrored, allocation_granularity};

pub use self::buffer::Buffer;
