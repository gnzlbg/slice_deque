//! A double-ended queue that `Deref`s into a slice.
//!
//! The double-ended queue in the standard library ([`VecDeque`]) is implemented
//! using a growable ring buffer (`0` represents uninitialized memory, and `T`
//! represents one enelemnt in the queue):
//!
//! ```rust
//! // [ 0 | 0 | 0 | T | T | T | 0 ]
//! //               ^:head  ^:tail
//! ```
//!
//! When the queue grows beyond the end of the allocated buffer, its tail wraps
//! around:
//!
//! ```rust
//! // [ T | T | 0 | T | T | T | T ]
//! //       ^:tail  ^:head
//! ```
//!
//! As a consequence, [`VecDeque`] cannot `Deref` into a slice, since its
//! elements do not, in general, occupy a contiguous memory region. This
//! complicates the implementation and its interface (for example, there is no
//! `as_slice` method, but [`as_slices`] returns a pair of slices) and has
//! negative performance consequences (e.g. need to account for wrap around
//! while iterating over the elements).
//!
//! This crates provides [`SliceDeque`], a double-ended queue implemented with
//! a growable *virtual* ring-buffer.
//!
//! A virtual ring-buffer implementation is very similar to the one used in
//! `VecDeque`. The main difference is that a virtual ring-buffer maps two
//! adjacent regions of virtual memory to the same region of physical memory:
//!
//! ```rust
//! // Virtual memory:
//! //
//! //  __________region_0_________ __________region_1_________
//! // [ 0 | 0 | 0 | T | T | T | 0 | 0 | 0 | 0 | T | T | T | 0 ]
//! //               ^:head  ^:tail
//! //
//! // Physical memory:
//! //
//! // [ 0 | 0 | 0 | T | T | T | 0 ]
//! //               ^:head  ^:tail
//! ```
//!
//! That is, both the virtual memory regions `0` and `1` above (top) map to the same
//! physical memory (bottom). Just like `VecDeque`, when the queue grows beyond the
//! end of the allocated physical memory region, the queue wraps around, and new
//! elements continue to be appended at the beginning of the queue. However, because
//! `SliceDeque` maps the physical memory to two adjacent memory regions, in virtual
//! memory space the queue maintais the ilusion of a contiguous memory layout:
//!
//! ```rust
//! // Virtual memory:
//! //
//! //  __________region_0_________ __________region_1_________
//! // [ T | T | 0 | T | T | T | T | T | T | 0 | T | T | T | T ]
//! //               ^:head              ^:tail
//! //
//! // Physical memory:
//! //
//! // [ T | T | 0 | T | T | T | T ]
//! //       ^:tail  ^:head
//! ```
//!
//! Since processes in many Operating Systems only deal with virtual memory
//! addresses, leaving the mapping to physical memory to the CPU Memory Management
//! Unit (MMU), [`SliceDeque`] is able to `Deref`s into a slice in those systems.
//!
//! This simplifies [`SliceDeque`]'s API and implementation, giving it a performance
//! advantage over [`VecDeque`] in some situations.
//!
//! In general, you can think of [`SliceDeque`] as a `Vec` with `O(1)` `pop_front`
//! and amortized `O(1)` `push_front` methods.
//!
//! The main drawbacks of [`SliceDeque`] are:
//!
//! * constrained platform support: by necessity [`SliceDeque`] must use the
//! platform-specific virtual memory facilities of the underlying operating
//! system. While [`SliceDeque`] can work on all major operating systems,
//! currently only `MacOS X` is supported.
//!
//! * no global allocator support: since the `Alloc`ator API does not support
//! virtual memory, to use platform-specific virtual memory support
//! [`SliceDeque`] must bypass the global allocator and talk directly to the
//! operating system. This can have negative performance consequences since
//! growing [`SliceDeque`] is always going to incur the cost of some system
//! calls.
//!
//! * capacity constrained by virtual memory facilities: [`SliceDeque`] must
//! allocate two adjacent memory regions that map to the same region of physical
//! memory. Most operating systems allow this operation to be performed
//! exclusively on memory pages (or memory allocations that are multiples of a
//! memory page). As a consequence, the smalles [`SliceDeque`] that can be
//! created has typically a capacity of 2 memory pages, and it can grow only to
//! capacities that are a multiple of a memory page.
//!
//! The main advantages of [`SliceDeque`] are:
//!
//! * nicer API: since it `Deref`s to a slice, all operations that work on
//! slices are available for `SliceDeque`.
//!
//! * efficient iteration: as efficient as for slices.
//!
//! * simpler serialization: since one can just serialize/deserialize a single slice.
//!
//! All in all, if your double-ended queues are small (smaller than a memory
//! page) or they get resized very often, `VecDeque` can perform better than
//! [`SliceDeque`]. Otherwise, [`SliceDeque`] typically performs better (see
//! the benchmarks), but platform support and global allocator bypass are two
//! reasons to weight in against its usage.
//!
//! [`VecDeque`]: https://doc.rust-lang.org/std/collections/struct.VecDeque.html
//! [`as_slices`]: https://doc.rust-lang.org/std/collections/struct.VecDeque.html#method.as_slices
//! [`SliceDeque`]: struct.SliceDeque.html

#![feature(nonzero, slice_get_slice, fused, core_intrinsics, shared, exact_size_is_empty,
           collections_range)]
#![cfg_attr(test, feature(conservative_impl_trait, const_atomic_usize_new))]
#![cfg_attr(feature = "cargo-clippy",
            allow(len_without_is_empty, shadow_reuse, cast_possible_wrap, cast_sign_loss))]

extern crate core;

use core::intrinsics::unlikely;

#[cfg(target_os = "macos")]
extern crate mach;

#[cfg(target_os = "linux")]
extern crate libc;

mod mirrored;
pub use mirrored::Buffer;

/// A double-ended queue that derefs into a slice.
///
/// It is implemented with a growable virtual ring buffer.
pub struct SliceDeque<T> {
    /// Index of the first element in the queue.
    head: usize,
    /// Index of one past the last element in the queue.
    tail: usize,
    /// Mirrored memory buffer.
    buf: Buffer<T>,
}

impl<T> SliceDeque<T> {
    /// Creates a new empty deque.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use slice_deque::SliceDeque;
    /// let deq = SliceDeque::new();
    /// # let o: SliceDeque<u32> = deq;
    /// ```
    #[inline]
    pub fn new() -> Self {
        Self {
            head: 0,
            tail: 0,
            buf: Buffer::new(),
        }
    }

    /// Create an empty deque with capacity to hold `n` elements.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use slice_deque::SliceDeque;
    /// let deq = SliceDeque::with_capacity(10);
    /// # let o: SliceDeque<u32> = deq;
    /// ```
    #[inline]
    pub fn with_capacity(n: usize) -> Self {
        unsafe {
            Self {
                head: 0,
                tail: 0,
                buf: Buffer::uninitialized(2 * n).expect("failed to allocate a buffer"),
            }
        }
    }

    /// Returns the number of elements that the deque can hold without reallocating.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use slice_deque::SliceDeque;
    /// let deq = SliceDeque::with_capacity(10);
    /// assert!(deq.capacity() >= 10);
    /// # let o: SliceDeque<u32> = deq;
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buf.len() / 2
    }

    /// Number of elements in the ring buffer.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::with_capacity(10);
    /// assert!(deq.len() == 0);
    /// deq.push_back(3);
    /// assert!(deq.len() == 1);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        let l = self.tail - self.head;
        debug_assert!(self.tail >= self.head);
        debug_assert!(l <= self.capacity());
        l
    }

    /// Is the ring buffer full ?
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::with_capacity(10);
    /// assert!(!deq.is_full());
    /// # let o: SliceDeque<u32> = deq;
    /// ```
    #[inline]
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    /// Extracts a slice containing the entire deque.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        unsafe {
            let ptr = self.buf.ptr();
            if unlikely(self.len() == 0) {
                return ::std::slice::from_raw_parts(ptr.get(), 0);
            }
            let ptr = ptr.get().offset(self.head as isize);
            ::std::slice::from_raw_parts(ptr, self.len())
        }
    }

    /// Extracts a mutable slice containing the entire deque.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe {
            let ptr = self.buf.ptr().get();
            if unlikely(self.len() == 0) {
                return ::std::slice::from_raw_parts_mut(ptr, 0);
            }
            let ptr = ptr.offset(self.head as isize);
            ::std::slice::from_raw_parts_mut(ptr, self.len())
        }
    }

    /// Reserves capacity for inserting at least `additional` elements without
    /// reallocating. Does nothing if the capacity is already sufficient.
    ///
    /// The collection always reserves memory in multiples of the page size.
    ///
    /// # Panics
    ///
    /// Panics if the new capacity overflows `usize`.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        let new_cap = self.capacity().checked_add(additional).expect("overflow");

        self.reserve_capacity(new_cap);
    }

    /// Reserves capacity for `new_capacity` elements. Does nothing if the
    /// capacity is already sufficient.
    fn reserve_capacity(&mut self, new_capacity: usize) {
        unsafe {
            if new_capacity <= self.capacity() {
                return;
            }

            let mut new_buffer = match Buffer::uninitialized(2 * new_capacity) {
                Err(()) => panic!("oom"),
                Ok(new_buffer) => new_buffer,
            };

            let len = self.len();
            // Move the elements from the current buffer
            // to the beginning of the new buffer:
            {
                let from_ptr = self.as_mut_ptr();
                let to_ptr = new_buffer.as_mut_slice().as_mut_ptr();
                ::core::ptr::copy_nonoverlapping(from_ptr, to_ptr, len);
            }

            // Exchange buffers
            ::std::mem::swap(&mut self.buf, &mut new_buffer);

            // Correct head and tail (we copied to the
            // beginning of the of the new buffer)
            self.head = 0;
            self.tail = len;
        }
    }

    /// Growth policy of the deque. The capacity is going to be a multiple of
    /// the page-size anyways, so we just double on growth.
    fn grow_policy(&self) -> usize {
        unsafe {
            if unlikely(self.capacity() == 0) {
                4
            } else {
                self.capacity() * 2
            }
        }
    }

    /// Grows the deque.
    fn grow(&mut self) {
        debug_assert!(self.is_full());

        let new_capacity = self.grow_policy();
        self.reserve_capacity(new_capacity);

        debug_assert!(!self.is_full());
    }

    /// Moves the deque head by `x`.
    unsafe fn move_head(&mut self, x: isize) {
        let head = self.head as isize;
        let mut new_head = head + x;
        let tail = self.tail as isize;
        let cap = self.capacity();
        debug_assert!(new_head <= tail);
        debug_assert!(tail - new_head <= cap as isize);

        // If the new head is negative we shift the range by capacity to
        // move it towards the second mirrored memory region.

        if unlikely(new_head < 0) {
            debug_assert!(tail < cap as isize);
            new_head += cap as isize;
            debug_assert!(new_head >= 0);
            self.tail += cap;
        }

        self.head = new_head as usize;
        debug_assert!(self.len() as isize == (tail - head) - x);
    }

    /// Moves the deque tail by `x`.
    unsafe fn move_tail(&mut self, x: isize) {
        let head = self.head as isize;
        let tail = self.tail as isize;
        let cap = self.capacity() as isize;
        let mut new_tail = tail + x;
        debug_assert!(head <= new_tail);
        debug_assert!(new_tail - head <= cap);

        // If the new tail falls of the mirrored region of virtual memory we
        // shift the range by -capacity to move it towards the first mirrored
        // memory region.

        if unlikely(new_tail >= 2 * cap) {
            debug_assert!(head >= cap);
            self.head -= cap as usize;
            new_tail -= cap as isize;
            debug_assert!(new_tail <= cap);
        }

        self.tail = new_tail as usize;
        debug_assert!(self.len() as isize == (tail - head) + x);
    }

    /// Provides a reference to the first element, or `None` if the deque is
    /// empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// assert_eq!(deq.front(), None);
    ///
    /// deq.push_back(1);
    /// deq.push_back(2);
    /// assert_eq!(deq.front(), Some(&1));
    /// deq.push_front(3);
    /// assert_eq!(deq.front(), Some(&3));
    /// ```
    #[inline]
    pub fn front(&self) -> Option<&T> {
        self.get(0)
    }

    /// Provides a mutable reference to the first element, or `None` if the
    /// deque is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// assert_eq!(deq.front(), None);
    ///
    /// deq.push_back(1);
    /// deq.push_back(2);
    /// assert_eq!(deq.front(), Some(&1));
    /// (*deq.front_mut().unwrap()) = 3;
    /// assert_eq!(deq.front(), Some(&3));
    /// ```
    #[inline]
    pub fn front_mut(&mut self) -> Option<&mut T> {
        self.get_mut(0)
    }

    /// Provides a reference to the last element, or `None` if the deque is
    /// empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// assert_eq!(deq.back(), None);
    ///
    /// deq.push_back(1);
    /// deq.push_back(2);
    /// assert_eq!(deq.back(), Some(&2));
    /// deq.push_front(3);
    /// assert_eq!(deq.back(), Some(&2));
    /// ```
    #[inline]
    pub fn back(&self) -> Option<&T> {
        let last_idx = self.len().wrapping_sub(1);
        self.get(last_idx)
    }

    /// Provides a mutable reference to the last element, or `None` if the
    /// deque is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// assert_eq!(deq.front(), None);
    ///
    /// deq.push_back(1);
    /// deq.push_back(2);
    /// assert_eq!(deq.back(), Some(&2));
    /// (*deq.back_mut().unwrap()) = 3;
    /// assert_eq!(deq.back(), Some(&3));
    /// ```
    #[inline]
    pub fn back_mut(&mut self) -> Option<&mut T> {
        let last_idx = self.len().wrapping_sub(1);
        self.get_mut(last_idx)
    }

    /// Prepends `value` to the deque.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// deq.push_front(1);
    /// deq.push_front(2);
    /// assert_eq!(deq.front(), Some(&2));
    /// ```
    #[inline]
    pub fn push_front(&mut self, value: T) {
        unsafe {
            if unlikely(self.is_full()) {
                self.grow();
            }

            self.move_head(-1);
            *self.get_mut(0).unwrap() = value;
        }
    }

    /// Appends `value` to the deque.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// deq.push_back(1);
    /// deq.push_back(3);
    /// assert_eq!(deq.back(), Some(&3));
    /// ```
    #[inline]
    pub fn push_back(&mut self, value: T) {
        unsafe {
            if unlikely(self.is_full()) {
                self.grow();
            }
            self.move_tail(1);
            let len = self.len();
            std::ptr::write(self.get_mut(len - 1).unwrap(), value);
        }
    }

    /// Removes the first element and returns it, or `None` if the deque is
    /// empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// deq.push_back(1);
    /// deq.push_back(2);
    ///
    /// assert_eq!(deq.pop_front(), Some(1));
    /// assert_eq!(deq.pop_front(), Some(2));
    /// assert_eq!(deq.pop_front(), None);
    /// ```
    #[inline]
    pub fn pop_front(&mut self) -> Option<T> {
        unsafe {
            let v = match self.get_mut(0) {
                None => return None,
                Some(v) => {
                    let mut o: T = ::std::mem::uninitialized();
                    ::std::mem::swap(v, &mut o);
                    o
                }
            };
            self.move_head(1);
            Some(v)
        }
    }

    /// Removes the last element from the deque and returns it, or `None` if it
    /// is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// assert_eq!(deq.pop_back(), None);
    /// deq.push_back(1);
    /// deq.push_back(3);
    /// assert_eq!(deq.pop_back(), Some(3));
    /// assert_eq!(deq.pop_back(), Some(1));
    /// assert_eq!(deq.pop_back(), None);
    /// ```
    #[inline]
    pub fn pop_back(&mut self) -> Option<T> {
        unsafe {
            let len = self.len();
            let v = match self.get_mut(len.wrapping_sub(1)) {
                None => return None,
                Some(v) => {
                    let mut o: T = ::std::mem::uninitialized();
                    ::std::mem::swap(v, &mut o);
                    o
                }
            };
            self.move_tail(-1);
            Some(v)
        }
    }

    /// Shrinks the capacity of the deque as much as possible.
    ///
    /// It will drop down as close as possible to the length, but because
    /// `SliceDeque` allocates memory in multiples of the page size the deque
    /// might still have capacity for inserting new elements without
    /// reallocating.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::with_capacity(15);
    /// // deq.extend(0..4); // TODO: extend
    /// assert!(deq.capacity() >= 15);
    /// deq.shrink_to_fit();
    /// assert!(deq.capacity() >= 4);
    /// # let o: SliceDeque<u32> = deq;
    /// ```
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        if self.is_empty() {
            return;
        }

        let mut new_vd = Self::with_capacity(self.len());
        unsafe {
            ::core::ptr::copy_nonoverlapping(self.as_mut_ptr(), new_vd.as_mut_ptr(), self.len());
        }
        new_vd.tail = self.len();
        ::std::mem::swap(self, &mut new_vd);
    }

    /// Shortens the deque by removing excess elements from the back.
    ///
    /// If `len` is greater than the VecDeque's current length, this has no
    /// effect.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use slice_deque::SliceDeque;
    ///
    /// let mut deq = SliceDeque::new();
    /// deq.push_back(5);
    /// deq.push_back(10);
    /// deq.push_back(15);
    /// assert_eq!(deq, [5, 10, 15]);
    /// deq.truncate(1);
    /// assert_eq!(deq, [5]);
    /// ```
    #[inline]
    pub fn truncate(&mut self, len: usize) {
        unsafe {
            while len < self.len() {
                // decrement tail before the drop_in_place(), so a panic on Drop
                // doesn't re-drop the just-failed value.
                self.tail -= 1;
                let len = self.len();
                core::ptr::drop_in_place(self.get_unchecked_mut(len));
            }
        }
    }

    /// Creates a draining iterator that removes the specified range in the deque
    /// and yields the removed items.
    ///
    /// Note 1: The element range is removed even if the iterator is only
    /// partially consumed or not consumed at all.
    ///
    /// Note 2: It is unspecified how many elements are removed from the vector
    /// if the `Drain` value is leaked.
    ///
    /// # Panics
    ///
    /// Panics if the starting point is greater than the end point or if
    /// the end point is greater than the length of the vector.
    ///
    /// # Examples
    ///
    /// ```
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// deq.push_back(1);
    /// deq.push_back(2);
    /// deq.push_back(3);
    /// let u: Vec<_> = deq.drain(1..).collect();
    /// assert_eq!(deq, &[1]);
    /// assert_eq!(u, &[2, 3]);
    ///
    /// // A full range clears the deque
    /// deq.drain(..);
    /// assert_eq!(deq, &[]);
    /// ```
    #[inline]
    pub fn drain<R>(&mut self, range: R) -> Drain<T>
    where
        R: ::std::collections::range::RangeArgument<usize>,
    {
        use std::collections::Bound::{Excluded, Included, Unbounded};
        // Memory safety
        //
        // When the Drain is first created, it shortens the length of
        // the source vector to make sure no uninitalized or moved-from elements
        // are accessible at all if the Drain's destructor never gets to run.
        //
        // Drain will ptr::read out the values to remove.
        // When finished, remaining tail of the vec is copied back to cover
        // the hole, and the vector length is restored to the new length.
        //
        let len = self.len();
        let start = match range.start() {
            Included(&n) => n,
            Excluded(&n) => n + 1,
            Unbounded => 0,
        };
        let end = match range.end() {
            Included(&n) => n + 1,
            Excluded(&n) => n,
            Unbounded => len,
        };
        assert!(start <= end);
        assert!(end <= len);

        unsafe {
            // set self.deq length's to start, to be safe in case Drain is leaked
            self.tail = self.head + start;;
            // Use the borrow in the IterMut to indicate borrowing behavior of the
            // whole Drain iterator (like &mut T).
            let range_slice = ::std::slice::from_raw_parts_mut(
                self.as_mut_ptr().offset(start as isize),
                end - start,
            );
            Drain {
                tail_start: end,
                tail_len: len - end,
                iter: range_slice.iter(),
                deq: ::std::ptr::Shared::from(self),
            }
        }
    }

    /// Removes all values from the deque.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// deq.push_back(1);
    /// deq.clear();
    /// assert!(deq.is_empty());
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.truncate(0);
    }

    /// Removes the element at `index` and return it in `O(1)` by swapping the
    /// last element into its place.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// assert_eq!(deq.swap_remove_back(0), None);
    /// deq.push_back(1);
    /// deq.push_back(2);
    /// deq.push_back(3);
    /// assert_eq!(deq, [1, 2, 3]);
    ///
    /// assert_eq!(deq.swap_remove_back(0), Some(1));
    /// assert_eq!(deq, [3, 2]);
    /// ```
    #[inline]
    pub fn swap_remove_back(&mut self, index: usize) -> Option<T> {
        let len = self.len();
        if self.is_empty() {
            None
        } else {
            self.swap(index, len - 1);
            self.pop_back()
        }
    }

    /// Removes the element at `index` and returns it in `O(1)` by swapping the
    /// first element into its place.
    ///
    /// # Examples
    ///
    /// ```
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// assert_eq!(deq.swap_remove_front(0), None);
    /// deq.push_back(1);
    /// deq.push_back(2);
    /// deq.push_back(3);
    /// assert_eq!(deq, [1, 2, 3]);
    ///
    /// assert_eq!(deq.swap_remove_front(2), Some(3));
    /// assert_eq!(deq, [2, 1]);
    /// ```
    #[inline]
    pub fn swap_remove_front(&mut self, index: usize) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            self.swap(index, 0);
            self.pop_front()
        }
    }

    /// Inserts an `element` at `index` within the deque, shifting all elements
    /// with indices greater than or equal to `index` towards the back.
    ///
    /// Element at index 0 is the front of the queue.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than deque's length
    ///
    /// # Examples
    ///
    /// ```
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// deq.push_back('a');
    /// deq.push_back('b');
    /// deq.push_back('c');
    /// assert_eq!(deq, &['a', 'b', 'c']);
    ///
    /// deq.insert(1, 'd');
    /// assert_eq!(deq, &['a', 'd', 'b', 'c']);
    /// ```
    #[inline]
    pub fn insert(&mut self, index: usize, element: T) {
        unsafe {
            let len = self.len();
            assert!(index <= len);

            if unlikely(self.is_full()) {
                self.grow();
            }

            let p = self.as_mut_ptr().offset(index as isize);
            ::std::ptr::copy(p, p.offset(1), len - index); // Shift elements
            ::std::ptr::write(p, element); // Overwritte
            self.move_tail(1);
        }
    }

    /// Removes and returns the element at position `index` within the deque,
    /// shifting all elements after it to the front.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// deq.push_back(1);
    /// deq.push_back(2);
    /// deq.push_back(3);
    /// assert_eq!(deq.remove(1), 2);
    /// assert_eq!(deq, [1, 3]);
    /// ```
    #[inline]
    pub fn remove(&mut self, index: usize) -> T {
        let len = self.len();
        assert!(index < len);
        unsafe {
            // copy element at pointer:
            let ptr = self.as_mut_ptr().offset(index as isize);
            let ret = ::std::ptr::read(ptr);
            // shift everything to the front overwriting the deque copy of the
            // element:
            ::std::ptr::copy(ptr.offset(1), ptr, len - index - 1);
            self.move_tail(-1);
            ret
        }
    }

    /// Splits the collection into two at the given index.
    ///
    /// Returns a newly allocated `Self`. `self` contains elements `[0, at)`,
    /// and the returned `Self` contains elements `[at, len)`.
    ///
    /// Note that the capacity of `self` does not change.
    ///
    /// # Panics
    ///
    /// Panics if `at > len`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// deq.push_back(1);
    /// deq.push_back(2);
    /// deq.push_back(3);
    /// let deq2 = deq.split_off(1);
    /// assert_eq!(deq, [1]);
    /// assert_eq!(deq2, [2, 3]);
    /// ```
    #[inline]
    pub fn split_off(&mut self, at: usize) -> Self {
        assert!(at <= self.len(), "`at` out of bounds");

        let other_len = self.len() - at;
        let mut other = Self::with_capacity(other_len);

        unsafe {
            self.move_tail(-(other_len as isize));
            other.move_tail(other_len as isize);

            ::std::ptr::copy_nonoverlapping(
                self.as_ptr().offset(at as isize),
                other.as_mut_ptr(),
                other.len(),
            );
        }
        other
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// That is, remove all elements `e` such that `f(&e)` returns `false`. This
    /// method operates in place and preserves the order of the retained
    /// elements.
    ///
    /// # Examples
    ///
    /// ```
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// deq.push_back(1);
    /// deq.push_back(2);
    /// deq.push_back(3);
    /// deq.push_back(4);
    /// deq.retain(|&x| x%2 == 0);
    /// assert_eq!(deq, [2, 4]);
    /// ```
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> bool,
    {
        let len = self.len();
        let mut del = 0;
        {
            let v = &mut **self;

            for i in 0..len {
                if !f(&v[i]) {
                    del += 1;
                } else if del > 0 {
                    v.swap(i - del, i);
                }
            }
        }
        if del > 0 {
            self.truncate(len - del);
        }
    }
    // fn place_back(&mut self) -> PlaceBack<T>
    // fn place_front(&mut self) -> PlaceFront<T>
}

impl<T> SliceDeque<T>
where
    T: Clone,
{
    /// Modifies the `SliceDeque` in-place so that `len()` is equal to
    /// `new_len`, either by removing excess elements or by appending clones of
    /// `value` to the back.
    ///
    /// # Examples
    ///
    /// ```
    /// # use slice_deque::SliceDeque;
    /// let mut deq = SliceDeque::new();
    /// deq.push_back(5);
    /// deq.push_back(10);
    /// deq.push_back(15);
    /// assert_eq!(deq, [5, 10, 15]);
    ///
    /// deq.resize(2, 0);
    /// assert_eq!(deq, [5, 10]);
    ///
    /// deq.resize(5, 20);
    /// assert_eq!(deq, [5, 10, 20, 20, 20]);
    /// ```
    pub fn resize(&mut self, new_len: usize, value: T) {
        let len = self.len();

        if new_len > len {
            self.reserve(new_len - len);
            while self.len() < new_len {
                self.push_back(value.clone());
            }
        } else {
            self.truncate(new_len);
        }
        debug_assert!(self.len() == new_len);
    }
}

impl<T: ::std::fmt::Debug> std::fmt::Debug for SliceDeque<T> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        write!(
            f,
            "[ len: {}, cap: {}, head: {}, tail: {} | {:?} ]",
            self.len(),
            self.capacity(),
            self.head,
            self.tail,
            self.as_slice()
        )
    }
}

impl<T> Drop for SliceDeque<T> {
    fn drop(&mut self) {
        self.clear();
    }
}

impl<T> core::ops::Deref for SliceDeque<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> core::ops::DerefMut for SliceDeque<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<T> Default for SliceDeque<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// A draining iterator for `SliceDeque<T>`.
///
/// This `struct` is created by the [`drain`] method on [`SliceDeque`].
///
/// [`drain`]: struct.SliceDeque.html#method.drain
/// [`SliceDeque`]: struct.SliceDeque.html
pub struct Drain<'a, T: 'a> {
    /// Index of tail to preserve
    tail_start: usize,
    /// Length of tail
    tail_len: usize,
    /// Current remaining range to remove
    iter: ::std::slice::Iter<'a, T>,
    /// A shared mutable pointer to the deque (with shared ownership).
    deq: ::std::ptr::Shared<SliceDeque<T>>,
}

impl<'a, T: 'a + ::std::fmt::Debug> ::std::fmt::Debug for Drain<'a, T> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        f.debug_tuple("Drain").field(&self.iter.as_slice()).finish()
    }
}

unsafe impl<'a, T: Sync> Sync for Drain<'a, T> {}
unsafe impl<'a, T: Send> Send for Drain<'a, T> {}

impl<'a, T> Iterator for Drain<'a, T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.iter
            .next()
            .map(|elt| unsafe { ::std::ptr::read(elt as *const _) })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, T> DoubleEndedIterator for Drain<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<T> {
        self.iter
            .next_back()
            .map(|elt| unsafe { ::std::ptr::read(elt as *const _) })
    }
}

impl<'a, T> Drop for Drain<'a, T> {
    fn drop(&mut self) {
        // exhaust self first
        while let Some(_) = self.next() {}

        if self.tail_len > 0 {
            unsafe {
                let source_deq = self.deq.as_mut();
                // memmove back untouched tail, update to new length
                let start = source_deq.len();
                let tail = self.tail_start;
                let src = source_deq.as_ptr().offset(tail as isize);
                let dst = source_deq.as_mut_ptr().offset(start as isize);
                ::std::ptr::copy(src, dst, self.tail_len);
                source_deq.move_tail(self.tail_len as isize);
            }
        }
    }
}

impl<'a, T> ExactSizeIterator for Drain<'a, T> {
    fn is_empty(&self) -> bool {
        self.iter.is_empty()
    }
}

impl<'a, T> ::std::iter::FusedIterator for Drain<'a, T> {}

macro_rules! __impl_slice_eq1 {
    ($Lhs: ty, $Rhs: ty) => {
        __impl_slice_eq1! { $Lhs, $Rhs, Sized }
    };
    ($Lhs: ty, $Rhs: ty, $Bound: ident) => {
        impl<'a, 'b, A: $Bound, B> PartialEq<$Rhs> for $Lhs where A: PartialEq<B> {
            #[inline]
            fn eq(&self, other: &$Rhs) -> bool { self[..] == other[..] }
        }
    }
}

__impl_slice_eq1! { SliceDeque<A>, SliceDeque<B> }
__impl_slice_eq1! { SliceDeque<A>, &'b [B] }
__impl_slice_eq1! { SliceDeque<A>, &'b mut [B] }
__impl_slice_eq1! { SliceDeque<A>, Vec<B> }

macro_rules! array_impls {
    ($($N: expr)+) => {
        $(
            // NOTE: some less important impls are omitted to reduce code bloat
            __impl_slice_eq1! { SliceDeque<A>, [B; $N] }
            __impl_slice_eq1! { SliceDeque<A>, &'b [B; $N] }
        )+
    }
}

array_impls! {
    0  1  2  3  4  5  6  7  8  9
        10 11 12 13 14 15 16 17 18 19
        20 21 22 23 24 25 26 27 28 29
        30 31 32
}

impl<T: Eq> Eq for SliceDeque<T> {}

impl<T: Clone> Clone for SliceDeque<T> {
    fn clone(&self) -> Self {
        let mut new = Self::with_capacity(self.len());
        for i in self.iter() {
            new.push_back(i.clone());
        }
        new
    }
    fn clone_from(&mut self, other: &Self) {
        self.clear();
        for i in other.iter() {
            self.push_back(i.clone());
        }
    }
}

impl<'a, T: Clone> From<&'a [T]> for SliceDeque<T> {
    fn from(s: &'a [T]) -> Self {
        let mut new = Self::with_capacity(s.len());
        for i in s {
            new.push_back(i.clone());
        }
        new
    }
}

impl<'a, T: Clone> From<&'a mut [T]> for SliceDeque<T> {
    fn from(s: &'a mut [T]) -> Self {
        let mut new = Self::with_capacity(s.len());
        for i in s {
            new.push_back(i.clone());
        }
        new
    }
}

// Extend<A>
// Extend<&'a T>: T: 'a + Copy
// IntoIterator
// FromIterator
// Ord

impl<T: ::std::hash::Hash> ::std::hash::Hash for SliceDeque<T> {
    #[inline]
    fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
        ::std::hash::Hash::hash(&**self, state)
    }
}

#[cfg(test)]
mod tests {
    use super::SliceDeque;
    use std::rc::Rc;
    use std::cell::RefCell;

    #[derive(Clone, Debug)]
    struct WithDrop {
        counter: Rc<RefCell<usize>>,
    }

    impl Drop for WithDrop {
        fn drop(&mut self) {
            *self.counter.borrow_mut() += 1;
        }
    }

    fn sizes_to_test() -> impl Iterator<Item = usize> {
        let sample = vec![
            // powers of 2
            2,
            4,
            8,
            16,
            32,
            64,
            128,
            /*
            256,
            512,
            1024,
            2048,
            4096,
            8192, 16_384, 32_768,  65_536, 131_072, 262_144,
            */
            /*
            // powers of 2 - 1 or primes
            1, 3, 7, 13, 17, 31, 61, 127, 257, 509, 1021, 2039, 4093,
            8191, 16_381, 32_749,  65_537, 131_071, 262_143, 4_194_301,
            // powers of 10
            10, 100, 1000, 10_000, 100_000, 1_000_000_usize,*/
        ];
        sample.into_iter()
    }

    fn linear_usize_deque(size: usize) -> SliceDeque<usize> {
        let mut v: SliceDeque<usize> = SliceDeque::new();
        for i in 0..size {
            v.push_back(i);
            assert_eq!(v.len(), i + 1);
            for j in 0..v.len() {
                assert_eq!(*v.get(j).unwrap(), j);
            }
        }
        assert_eq!(v.len(), size);
        for i in 0..size {
            assert_eq!(*v.get(i).unwrap(), i);
        }
        v
    }

    fn constant_deque<T: Clone + ::std::fmt::Debug>(size: usize, val: &T) -> SliceDeque<T> {
        let mut v: SliceDeque<T> = SliceDeque::with_capacity(size);
        for i in 0..size {
            let copy = val.clone();
            v.push_back(copy);
            assert_eq!(v.len(), i + 1);
        }
        assert_eq!(v.len(), size);
        v
    }

    #[test]
    fn get() {
        let mut deq = SliceDeque::new();
        deq.push_back(3);
        deq.push_back(4);
        deq.push_back(5);
        assert_eq!(deq.get(1), Some(&4));
    }

    #[test]
    fn get_mut() {
        let mut deq = SliceDeque::new();
        deq.push_back(3);
        deq.push_back(4);
        deq.push_back(5);
        assert_eq!(deq.get(1), Some(&4));
        if let Some(elem) = deq.get_mut(1) {
            *elem = 7;
        }
        assert_eq!(deq[1], 7);
    }

    #[test]
    fn is_empty() {
        let mut deq = SliceDeque::new();
        assert!(deq.is_empty());
        deq.push_back(4);
        assert!(!deq.is_empty());
        deq.pop_front();
        assert!(deq.is_empty());
    }

    #[test]
    fn push_pop_front() {
        for size in sizes_to_test() {
            let mut v: SliceDeque<usize> = SliceDeque::new();
            for i in 0..size {
                v.push_front(i);
                assert_eq!(v.len(), i + 1);
                for j in 0..v.len() {
                    assert_eq!(*v.get(v.len() - j - 1).unwrap(), j);
                }
            }
            assert_eq!(v.len(), size);
            for i in 0..size {
                assert_eq!(*v.get(i).unwrap(), size - i - 1);
            }
            for i in 0..size {
                assert_eq!(v.len(), size - i);
                v.pop_front();
                for j in 0..v.len() {
                    assert_eq!(*v.get(v.len() - j - 1).unwrap(), j);
                }
            }
            assert_eq!(v.len(), 0);
        }
    }

    #[test]
    fn push_pop_back() {
        for size in sizes_to_test() {
            let mut v = linear_usize_deque(size);
            for i in 0..size {
                assert_eq!(v.len(), size - i);
                v.pop_back();
                for j in 0..v.len() {
                    assert_eq!(*v.get(j).unwrap(), j);
                }
            }
            assert_eq!(v.len(), 0);
        }
    }

    #[test]
    fn all_head_tails() {
        for size in sizes_to_test() {
            let mut v = linear_usize_deque(size);
            let permutations = 6 * v.capacity();

            // rotate from left to right
            for _ in 0..permutations {
                v.push_back(0);
                for j in (0..v.len() - 1).rev() {
                    *v.get_mut(j + 1).unwrap() = *v.get(j).unwrap();
                }
                v.pop_front();
                assert_eq!(v.len(), size);
                for k in 0..size {
                    assert_eq!(*v.get(k).unwrap(), k);
                }
            }

            // rotate from right to left
            for _ in 0..permutations {
                v.push_front(0);
                for j in 0..v.len() - 1 {
                    *v.get_mut(j).unwrap() = *v.get(j + 1).unwrap()
                }
                v.pop_back();
                assert_eq!(v.len(), size);
                for k in 0..size {
                    assert_eq!(*v.get(k).unwrap(), k);
                }
            }
        }
    }

    #[test]
    fn drop() {
        for size in sizes_to_test() {
            let mut counter = Rc::new(RefCell::new(0));
            let val = WithDrop {
                counter: counter.clone(),
            };
            {
                let _v = constant_deque(size, &val);
            }
            assert_eq!(*counter.borrow(), size);
        }
    }

    #[test]
    fn clear() {
        for size in sizes_to_test() {
            println!("s: {}", size);
            let mut counter = Rc::new(RefCell::new(0));
            let val = WithDrop {
                counter: counter.clone(),
            };
            assert_eq!(*counter.borrow(), 0);
            let mut v = constant_deque(size, &val);
            assert_eq!(*counter.borrow(), 0);
            v.clear();
            assert_eq!(*counter.borrow(), size);
            assert_eq!(v.len(), 0);
        }
    }

    #[test]
    fn resize() {
        for size in sizes_to_test() {
            let mut v = linear_usize_deque(size);
            let mut v_ref = linear_usize_deque(size / 2);
            v.resize(size / 2, 0);
            assert_eq!(v.len(), size / 2);
            assert_eq!(v.as_slice(), v_ref.as_slice());
            while v_ref.len() < size {
                v_ref.push_back(3);
            }
            v.resize(size, 3);
            assert_eq!(v.len(), size);
            assert_eq!(v_ref.len(), size);
            assert_eq!(v.as_slice(), v_ref.as_slice());

            v.resize(0, 3);
            assert_eq!(v.len(), 0);

            v.resize(size, 3);
            let v_ref = constant_deque(size, &3);
            assert_eq!(v.len(), size);
            assert_eq!(v_ref.len(), size);
            assert_eq!(v.as_slice(), v_ref.as_slice());
        }
    }

    #[test]
    fn default() {
        let d = SliceDeque::<u8>::default();
        let r = SliceDeque::<u8>::new();
        assert_eq!(d.as_slice(), r.as_slice());
    }

    #[test]
    fn shrink_to_fit() {
        let page_size = 4096;
        for size in sizes_to_test() {
            let mut deq = constant_deque(size, &(3 as u8));
            let old_cap = deq.capacity();
            deq.resize(size / 4, 3);
            deq.shrink_to_fit();
            if size <= page_size {
                assert_eq!(deq.capacity(), old_cap);
            } else {
                assert!(deq.capacity() < old_cap);
            }
        }
    }

    #[test]
    fn iter() {
        let mut deq = SliceDeque::new();
        deq.push_back(5);
        deq.push_back(3);
        deq.push_back(4);
        let b: &[_] = &[&5, &3, &4];
        let c: Vec<&i32> = deq.iter().collect();
        assert_eq!(&c[..], b);
    }

    #[test]
    fn iter_mut() {
        let mut deq = SliceDeque::new();
        deq.push_back(5);
        deq.push_back(3);
        deq.push_back(4);
        for num in deq.iter_mut() {
            *num = *num - 2;
        }
        let b: &[_] = &[&mut 3, &mut 1, &mut 2];
        assert_eq!(&deq.iter_mut().collect::<Vec<&mut i32>>()[..], b);
    }

    #[test]
    fn hash() {
        use std::collections::HashMap;
        let mut hm: HashMap<SliceDeque<u32>, u32> = HashMap::new();
        let mut a = SliceDeque::new();
        a.push_back(1);
        a.push_back(2);
        hm.insert(a.clone(), 3);
        let b = SliceDeque::new();
        assert_eq!(hm.get(&a), Some(&3));
        assert_eq!(hm.get(&b), None);
    }
}
