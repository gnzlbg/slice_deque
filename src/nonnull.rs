//! Implements `std::ptr::NonNull` but without optimizations.
use super::fmt;

/// Like `std::ptr::NonNull` but without optimizations.
pub struct NonNull<T: ?Sized> {
    /// A pointer that is never null.
    pointer: *const T,
}

impl<T: ?Sized> fmt::Debug for NonNull<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&self.as_ptr(), f)
    }
}

impl<T: ?Sized> NonNull<T> {
    /// Creates a new `NonNull`.
    ///
    /// # Safety
    ///
    /// `ptr` must be non-null.
    pub unsafe fn new_unchecked(ptr: *mut T) -> Self {
        Self { pointer: ptr }
    }

    /// Acquires the underlying `*mut` pointer.
    pub fn as_ptr(self) -> *mut T {
        self.pointer as *mut T
    }

    /// Mutably dereferences the content.
    ///
    /// The resulting lifetime is bound to self so this behaves "as if"
    /// it were actually an instance of T that is getting borrowed. If a longer
    /// (unbound) lifetime is needed, use `&mut *my_ptr.as_ptr()`.
    pub unsafe fn as_mut(&mut self) -> &mut T {
        &mut *self.as_ptr()
    }
}

impl<T: ?Sized> Clone for NonNull<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for NonNull<T> {}

impl<T: ?Sized> fmt::Pointer for NonNull<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&self.as_ptr(), f)
    }
}

impl<'a, T: ?Sized> From<&'a mut T> for NonNull<T> {
    fn from(reference: &'a mut T) -> Self {
        Self {
            pointer: reference as *const T,
        }
    }
}

impl<'a, T: ?Sized> From<&'a T> for NonNull<T> {
    fn from(reference: &'a T) -> Self {
        Self {
            pointer: reference as *const T,
        }
    }
}
