//! Virtual Ring Buffer

#[cfg(target_os = "macos")]
extern crate mach;

mod mirrored;
pub use mirrored::Buffer;
