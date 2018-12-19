extern crate bh_alloc;
#[macro_use]
extern crate afl;
extern crate arbitrary;
extern crate slice_deque;

#[global_allocator]
static ALLOC: bh_alloc::BumpAlloc = bh_alloc::BumpAlloc::INIT;

use arbitrary::*;
use slice_deque::SliceDeque;

/// A `SliceDeque<T>` model
///
/// This type mimics the semantics of `SliceDeque<T>` while being 'obviously
/// correct' enough to serve as a `QuickCheck` model. What is a SliceDeque? Well,
/// it's a queue that supports efficient push/pop from both the back and front
/// of the queue. Efficiency is of no interest to us and we'll just abuse a Vec,
/// much like with [`PropHashMap`].
pub struct PropSliceDeque<T> {
    data: Vec<T>,
}

impl<T> Default for PropSliceDeque<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> PropSliceDeque<T> {
    /// Construct a new `PropSliceDeque<T>`
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Push a value onto the back of `PropSliceDeque<T>`
    ///
    /// This is like to [`SliceDeque::push_back`]
    pub fn push_back(&mut self, value: T) {
        self.data.push(value)
    }

    /// Pop a value from the back of `PropSliceDeque<T>`, if one exists
    ///
    /// This is like to [`SliceDeque::pop_back`]
    pub fn pop_back(&mut self) -> Option<T> {
        self.data.pop()
    }

    /// Push a value to the front of `PropSliceDeque<T>`.
    ///
    /// This is like to [`SliceDeque::push_front`]
    pub fn push_front(&mut self, value: T) {
        self.data.insert(0, value);
    }

    /// Insert a value at the given index into `PropSliceDeque<T>`.
    ///
    /// This is like to [`SliceDeque::insert`]
    pub fn insert(&mut self, index: usize, value: T) {
        self.data.insert(index, value);
    }

    /// Remove and return a value from the given index `PropSliceDeque<T>`.
    ///
    /// This is like to [`SliceDeque::remove`]
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index < self.data.len() {
            Some(self.data.remove(index))
        } else {
            None
        }
    }

    /// Pop a value from the front of `PropSliceDeque<T>`, if one exists
    ///
    /// This is like to [`SliceDeque::pop_front`]
    pub fn pop_front(&mut self) -> Option<T> {
        if self.data.is_empty() {
            None
        } else {
            let val = self.data.remove(0);
            Some(val)
        }
    }

    /// Clear all contents of `PropSliceDeque`
    ///
    /// This is like to [`SliceDeque::clear`]
    pub fn clear(&mut self) {
        self.data.clear()
    }

    /// Provide a reference to the front element, if one exists
    ///
    /// This is like to [`SliceDeque::front`]
    pub fn front(&mut self) -> Option<&T> {
        if self.data.is_empty() {
            None
        } else {
            let val = &self.data[0];
            Some(val)
        }
    }

    /// Provide a reference to the back element, if one exists
    ///
    /// This is like to [`SliceDeque::back`]
    pub fn back(&mut self) -> Option<&T> {
        if self.data.is_empty() {
            None
        } else {
            let len = self.data.len();
            let val = &self.data[len - 1];
            Some(val)
        }
    }

    /// Replace an element at the given index with the back element, return the
    /// replaced element
    ///
    /// This is like to [`SliceDeque::swap_remove_back`]
    pub fn swap_remove_back(&mut self, index: usize) -> Option<T> {
        if self.data.is_empty() {
            None
        } else if self.data.len() == 1 {
            self.pop_back()
        } else if index < self.data.len() {
            let back = self.data.len() - 1;
            self.data.swap(index, back);
            self.pop_back()
        } else {
            None
        }
    }

    /// Return the number of elements in `PropSliceDeque<T>`
    ///
    /// This is like to [`SliceDeque::len`]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Return true if the PropSliceDeque is empty, else false
    ///
    /// This is like to [`SliceDeque::is_empty`]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// The `Op<T>` defines the set of operations that are available against
/// `SliceDeque<K, V>` and `PropSliceDeque<T>`. Some map directly to functions
/// available on the types, others require a more elaborate interpretation step.
#[derive(Clone, Debug)]
pub enum Op<T> {
    /// This operation triggers `SliceDeque::shrink_to_fit`
    ShrinkToFit,
    /// This operation triggers `SliceDeque::clear`
    Clear,
    /// This operation triggers `SliceDeque::push_back`
    PushBack(T),
    /// This operation triggers `SliceDeque::pop_back`
    PopBack,
    /// This operation triggers `SliceDeque::push_front`
    PushFront(T),
    /// This operation triggers `SliceDeque::pop_front`
    PopFront,
    /// This operation triggers `SliceDeque::insert`
    Insert(usize, T),
    /// This operation triggers `SliceDeque::remove`
    Remove(usize),
    /// This operation triggers `SliceDeque::swap_remove_back`
    SwapRemoveBack(usize),
}

impl<T> Arbitrary for Op<T>
where
    T: Clone + Send + Arbitrary,
{
    fn arbitrary<U>(u: &mut U) -> Result<Self, U::Error>
    where
        U: Unstructured + ?Sized,
    {
        // ================ WARNING ================
        //
        // `total_enum_fields` is a goofy annoyance but it should match
        // _exactly_ the number of fields available in `Op<T>`. If it
        // does not then we'll fail to generate `Op` variants for use in our
        // QC tests.
        let total_enum_fields = 9;
        let variant: u8 = Arbitrary::arbitrary(u)?;
        let op = match variant % total_enum_fields {
            0 => {
                let t: T = Arbitrary::arbitrary(u)?;
                Op::PushBack(t)
            }
            1 => Op::PopBack,
            2 => {
                let t: T = Arbitrary::arbitrary(u)?;
                Op::PushFront(t)
            }
            3 => Op::PopFront,
            4 => Op::Clear,
            5 => Op::ShrinkToFit,
            6 => {
                let idx: usize = Arbitrary::arbitrary(u)?;
                let t: T = Arbitrary::arbitrary(u)?;
                Op::Insert(idx, t)
            }
            7 => {
                let idx: usize = Arbitrary::arbitrary(u)?;
                Op::Remove(idx)
            }
            8 => {
                let idx: usize = Arbitrary::arbitrary(u)?;
                Op::SwapRemoveBack(idx)
            }
            _ => unreachable!(),
        };
        Ok(op)
    }
}

fn main() {
    fuzz!(|data: &[u8]| {
        if let Ok(mut ring) = FiniteBuffer::new(data, 65_563) {
            let capacity: u8 = if let Ok(cap) = Arbitrary::arbitrary(&mut ring) {
                cap
            } else {
                return;
            };
            let mut model: PropSliceDeque<u8> = PropSliceDeque::new();
            let mut sut: SliceDeque<u8> = SliceDeque::with_capacity(capacity as usize);
            while let Ok(op) = Arbitrary::arbitrary(&mut ring) {
                match op {
                    Op::Clear => {
                        // Clearing a SliceDeque removes all elements but keeps
                        // the memory around for reuse. That is, the length
                        // should drop to zero but the capacity will remain the
                        // same.
                        let prev_cap = sut.capacity();
                        sut.clear();
                        model.clear();
                        assert_eq!(0, sut.len());
                        assert_eq!(sut.len(), model.len());
                        assert_eq!(prev_cap, sut.capacity());
                    }
                    Op::ShrinkToFit => {
                        // NOTE There is no model behaviour here
                        //
                        // After a shrink the capacity may or may not shift from
                        // the passed arg `capacity`. But, the capacity of the
                        // SliceDeque should never grow after a shrink.
                        //
                        // Similarly, the length of the SliceDeque prior to a
                        // shrink should match the length after a shrink.
                        let prev_len = sut.len();
                        let prev_cap = sut.capacity();
                        sut.shrink_to_fit();
                        assert_eq!(prev_len, sut.len());
                        assert!(sut.capacity() <= prev_cap);
                    }
                    Op::PushBack(t) => {
                        sut.push_back(t);
                        model.push_back(t);
                    }
                    Op::PushFront(t) => {
                        sut.push_front(t);
                        model.push_front(t);
                    }
                    Op::PopFront => {
                        let sut_res = sut.pop_front();
                        let model_res = model.pop_front();
                        assert_eq!(sut_res, model_res);
                    }
                    Op::PopBack => {
                        let sut_res = sut.pop_back();
                        let model_res = model.pop_back();
                        assert_eq!(sut_res, model_res);
                    }
                    Op::Insert(idx, t) => {
                        let scaled_idx = if !model.is_empty() {
                            idx % model.len()
                        } else {
                            0
                        };
                        model.insert(scaled_idx, t);
                        sut.insert(scaled_idx, t);
                    }
                    Op::Remove(idx) => {
                        let sut_res = sut.remove(idx);
                        let model_res = model.remove(idx);
                        assert_eq!(sut_res, model_res);
                    } // TODO(blt) the SUT and model deviate for unknown
                    // reasons. Perfect opportunity to extend the QC powered
                    // fuzzer notion to aid debugging. Right now the Op list
                    // that trigger is... very big.
                    //
                    Op::SwapRemoveBack(_idx) => {
                        //     let sut_res = sut.swap_remove_back(idx);
                        //     let model_res = model.swap_remove_back(idx);
                        //     assert_eq!(sut_res, model_res);
                    }
                }
                // Check invariants
                //
                // `SliceDeque<T>` defines the return of `capacity` as being
                // "the number of elements the map can hold without
                // reallocating". Unlike `HashMap<K, V>` there is no
                // discussion of bounds. This implies that:
                //
                // * the SliceDeque capacity must always be at least the
                // length of the model
                assert!(sut.capacity() >= model.len());
                // The length of the SUT must always be exactly the length
                // of the model.
                assert_eq!(sut.len(), model.len());
                // If the SUT is empty then the model must also be.
                assert_eq!(sut.is_empty(), model.is_empty());
                // The front of the SUT must always be equivalent to the
                // front of the model.
                assert_eq!(sut.front(), model.front());
                // The back of the SUT must always be equivalent to the
                // back of the model.
                assert_eq!(sut.back(), model.back());
            }
        }
    })
}
