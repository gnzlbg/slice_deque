//! Macros and utilities.

/// Small Ascii String. Used to write errors in `no_std` environments.
pub struct TinyAsciiString {
    /// A buffer for the ascii string
    buf: [u8; 512],
}

impl TinyAsciiString {
    /// Creates a new string initialized to zero.
    pub fn new() -> Self {
        Self { buf: [0_u8; 512] }
    }
    /// Converts the Tiny Ascii String to an UTF-8 string (unchecked).
    pub unsafe fn as_str(&self) -> &str {
        ::str::from_utf8_unchecked(&self.buf)
    }
}

impl ::fmt::Write for TinyAsciiString {
    fn write_str(&mut self, s: &str) -> Result<(), ::fmt::Error> {
        for (idx, b) in s.bytes().enumerate() {
            if let Some(mut v) = self.buf.get_mut(idx) {
                *v = b;
            } else {
                return Err(::fmt::Error);
            }
        }
        Ok(())
    }
}

macro_rules! tiny_str {
    ($($t:tt)*) => (
        {
            use ::fmt::Write;
            let mut s: ::macros::TinyAsciiString = ::macros::TinyAsciiString::new();
            write!(&mut s, $($t)*).unwrap();
            s
        }
    )
}
