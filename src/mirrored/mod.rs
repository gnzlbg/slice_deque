//! Mirrored memory buffer.
mod buffer;

#[cfg(target_os = "macos")]
mod darwin;

#[cfg(target_os = "macos")]
use self::darwin::*;

pub use self::buffer::Buffer;
