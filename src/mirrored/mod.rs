//! Mirrored memory buffer.
use super::*;

mod buffer;
pub use self::buffer::Buffer;

mod alloc;
pub use self::alloc::release;
