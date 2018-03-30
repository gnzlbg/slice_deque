//! Implements a mirrored memory buffer.

use super::alloc::{allocate_mirrored, allocation_granularity,
                   deallocate_mirrored};
use super::*;
use ptr::NonNull;

/// Number of required memory allocation units to hold `bytes`.
fn no_required_allocation_units(bytes: usize) -> usize {
    let r = (bytes / allocation_granularity()).max(1);
    if r % 2 == 0 {
        r
    } else {
        r + 1
    }
}

/// Mirrored memory buffer of length `len`.
///
/// The buffer elements in range `[0, len/2)` are mirrored into the range
/// `[len/2, len)`.
pub struct Buffer<T> {
    /// Pointer to the first element in the buffer.
    ptr: NonNull<T>,
    /// Length of the buffer:
    ///
    /// * it is always a multiple of 2
    /// * the elements in range `[0, len/2)` are mirrored into the range
    /// `[len/2, len)`.
    len: usize,
}

impl<T> Buffer<T> {
    /// Number of elements in the buffer.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Is the buffer empty?
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Pointer to the first element in the buffer.
    pub unsafe fn ptr(&self) -> NonNull<T> {
        self.ptr
    }

    /// Interprets contents as a slice.
    ///
    /// Warning: Some memory might be uninitialized.
    pub unsafe fn as_slice(&self) -> &[T] {
        slice::from_raw_parts(self.ptr.as_ptr(), self.len())
    }

    /// Interprets contents as a mut slice.
    ///
    /// Warning: Some memory might be uninitialized.
    pub unsafe fn as_mut_slice(&mut self) -> &mut [T] {
        slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len())
    }

    /// Interprets content as a slice and access the `i`-th element.
    ///
    /// Warning: The memory of the `i`-th element might be uninitialized.
    pub unsafe fn get(&self, i: usize) -> &T {
        &self.as_slice()[i]
    }

    /// Interprets content as a mut slice and access the `i`-th element.
    ///
    /// Warning: The memory of the `i`-th element might be uninitialized.
    pub unsafe fn get_mut(&mut self, i: usize) -> &mut T {
        &mut self.as_mut_slice()[i]
    }

    /// Creates a new empty `Buffer`.
    pub fn new() -> Self {
        // Zero-sized elements are not supported yet:
        assert!(mem::size_of::<T>() > 0);
        // Here `ptr` is initialized to a magic value but `len == 0`
        // will ensure that it is never dereferenced in this state.
        unsafe {
            Self {
                ptr: NonNull::new_unchecked(usize::max_value() as *mut T),
                len: 0,
            }
        }
    }

    /// Creates a new empty `Buffer` from a `ptr` and a `len`.
    ///
    /// # Panics
    ///
    /// If `ptr` is null.
    pub unsafe fn from_raw_parts(ptr: *mut T, len: usize) -> Self {
        // Zero-sized types are not supported yet:
        assert!(mem::size_of::<T>() > 0);
        assert!(!ptr.is_null());
        Self {
            ptr: NonNull::new_unchecked(ptr),
            len,
        }
    }

    /// Total number of bytes in the buffer (including mirrored memory).
    fn size_in_bytes(len: usize) -> usize {
        no_required_allocation_units(len * mem::size_of::<T>())
            * allocation_granularity()
    }

    /// Create a mirrored buffer containing `len` `T`s where the first half of
    /// the buffer is mirrored into the second half.
    pub fn uninitialized(len: usize) -> Result<Self, ()> {
        // Zero-sized types are not supported yet:
        assert!(mem::size_of::<T>() > 0);
        // The alignment requirements of `T` must be smaller than the
        // allocation granularity.
        assert!(mem::align_of::<T>() <= allocation_granularity());
        // To split the buffer in two halfs the number of elements must be a
        // multiple of two, and greater than zero to be able to mirror
        // something.
        if len == 0 {
            return Ok(Self::new());
        }
        assert!(len % 2 == 0);

        // How much memory we need:
        let alloc_size = Self::size_in_bytes(len);
        debug_assert!(alloc_size > 0);
        debug_assert!(alloc_size % 2 == 0);
        debug_assert!(alloc_size % allocation_granularity() == 0);

        let ptr = allocate_mirrored(alloc_size)?;

        Ok(Self {
            ptr: unsafe { NonNull::new_unchecked(ptr as *mut T) },
            len: alloc_size / mem::size_of::<T>(),
        })
    }
}

impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        if self.is_empty() {
            return;
        }

        let buffer_size_in_bytes = Self::size_in_bytes(self.len());
        let first_half_ptr = self.ptr.as_ptr() as *mut u8;
        unsafe { deallocate_mirrored(first_half_ptr, buffer_size_in_bytes) };
    }
}

impl<T> Clone for Buffer<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        unsafe {
            let mid = self.len() / 2;
            let mut c = Self::uninitialized(self.len())
                .expect("allocating a new mirrored buffer failed");
            let (from, _) = self.as_slice().split_at(mid);
            {
                let (to, _) = c.as_mut_slice().split_at_mut(mid);
                to[..mid].clone_from_slice(&from[..mid]);
            }
            c
        }
    }
}

impl<T> Default for Buffer<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let a = Buffer::<u64>::new();
        assert!(a.is_empty());
    }

    fn test_alloc(size: usize) {
        unsafe {
            let mut a = Buffer::<u64>::uninitialized(size).unwrap();
            let sz = a.len();
            assert!(sz >= size);
            assert_eq!(
                sz,
                Buffer::<u64>::size_in_bytes(size) / mem::size_of::<u64>()
            );

            for i in 0..sz / 2 {
                *a.get_mut(i) = i as u64;
            }

            let (first_half_mut, second_half_mut) =
                a.as_mut_slice().split_at_mut(sz / 2);

            let mut c = 0;
            for (i, j) in first_half_mut.iter().zip(second_half_mut) {
                assert_eq!(i, j);
                c += 1;
            }
            assert_eq!(c, sz / 2);
        }
    }

    #[test]
    fn allocations() {
        let elements_per_alloc_unit =
            allocation_granularity() / mem::size_of::<u64>();
        let sizes = [
            8,
            elements_per_alloc_unit / 2,
            elements_per_alloc_unit,
            elements_per_alloc_unit * 4,
        ];
        for &i in &sizes {
            test_alloc(i);
        }
    }

    #[test]
    fn no_alloc_units_required() {
        // Up to the allocation unit size we always need two allocation units
        assert_eq!(
            no_required_allocation_units(allocation_granularity() / 4),
            2
        );
        assert_eq!(
            no_required_allocation_units(allocation_granularity() / 2),
            2
        );
        assert_eq!(
            no_required_allocation_units(allocation_granularity()),
            2
        );
        assert_eq!(
            no_required_allocation_units(2 * allocation_granularity()),
            2
        );
        // For sizes larger than the allocation units we always round up to the
        // next even number of allocation units:
        assert_eq!(
            no_required_allocation_units(3 * allocation_granularity()),
            4
        );
        assert_eq!(
            no_required_allocation_units(4 * allocation_granularity()),
            4
        );
        assert_eq!(
            no_required_allocation_units(5 * allocation_granularity()),
            6
        );
    }
}
