# Slice Deque

[![crates.io version][crate-shield]][crate] [![Travis build status][travis-shield]][travis] [![Appveyor build status][appveyor-shield]][appveyor] [![Coveralls.io code coverage][coveralls-shield]][coveralls] [![Docs][docs-shield]][docs] [![License][license-shield]][license]

> A double-ended queue that `Deref`s into a slice.

The double-ended queue in the standard library ([`VecDeque`]) is implemented
using a growable ring buffer (`0` represents uninitialized memory, and `T`
represents one enelemnt in the queue):

```rust
// [ 0 | 0 | 0 | T | T | T | 0 ]
//               ^:head  ^:tail
```

When the queue grows beyond the end of the allocated buffer, its tail wraps
around:

```rust
// [ T | T | 0 | T | T | T | T ]
//       ^:tail  ^:head
```

As a consequence, [`VecDeque`] cannot `Deref` into a slice, since its elements
do not, in general, occupy a contiguous memory region. This complicates the
implementation and its interface (for example, there is no `as_slice` method -
the [`as_slices`] method returns a pair of slices) and has negative performance
consequences (e.g. need to account for wrap around while iterating over the
elements).

This crates provides [`SliceDeque`], a double-ended queue implemented with
a growable *virtual* ring-buffer.

A virtual ring-buffer implementation is very similar to the one used in
`VecDeque`. The main difference is that a virtual ring-buffer maps two
adjacent regions of virtual memory to the same region of physical memory:

```rust
// Virtual memory:
//
//  __________region_0_________ __________region_1_________
// [ 0 | 0 | 0 | T | T | T | 0 | 0 | 0 | 0 | T | T | T | 0 ]
//               ^:head  ^:tail
//
// Physical memory:
//
// [ 0 | 0 | 0 | T | T | T | 0 ]
//               ^:head  ^:tail
```

That is, both the virtual memory regions `0` and `1` above (top) map to the same
physical memory (bottom). Just like `VecDeque`, when the queue grows beyond the
end of the allocated physical memory region, the queue wraps around, and new
elements continue to be appended at the beginning of the queue. However, because
`SliceDeque` maps the physical memory to two adjacent memory regions, in virtual
memory space the queue maintais the ilusion of a contiguous memory layout:

```rust
// Virtual memory:
//
//  __________region_0_________ __________region_1_________
// [ T | T | 0 | T | T | T | T | T | T | 0 | T | T | T | T ]
//               ^:head              ^:tail
//
// Physical memory:
//
// [ T | T | 0 | T | T | T | T ]
//       ^:tail  ^:head
```

Since processes in many Operating Systems only deal with virtual memory
addresses, leaving the mapping to physical memory to the CPU Memory Management
Unit (MMU), [`SliceDeque`] is able to `Deref`s into a slice in those systems.

This simplifies [`SliceDeque`]'s API and implementation, giving it a performance
advantage over [`VecDeque`] in some situations. 

In general, you can think of [`SliceDeque`] as a `Vec` with `O(1)` `pop_front`
and amortized `O(1)` `push_front` methods.

The main drawbacks of [`SliceDeque`] are:

* constrained platform support: [`SliceDeque`] requires an operating system with
virtual memory support. While [`SliceDeque`] can be made to work on all major
operating systems (they all have supported virtual-memory forever),
platform-specific code is required for this. Currently, the following operating
systems are supported:

    * `MacOS X`
    * `Linux`
    * `Windows`

* no global allocator support: [`SliceDeque`] directly uses OS-specific APIs, so
its behavior is independent of the global allocator.

* capacity constrained by virtual memory facilities: [`SliceDeque`] must
allocate two adjacent virtual memory regions that map to the same region of
physical memory. Most operating systems allow this operation to be performed
exclusively on memory pages or similar. As a consequence, the smallest capacity
that a [`SliceDeque`] can have is a memory page on `Linux` and `MacOSX` (4kB,
that is, `512` `u64`s), and an "allocation granularity" on Windows (64kB, that
is, `8192` `u64`s).

The main advantages of [`SliceDeque`] are:

* nicer API: since it `Deref`s to a slice, all operations that work on
slices are available for `SliceDeque`.

* efficient: as efficient as a slice (iteration, sorting, etc.).

When should I prefer [`VecDeque`] over [`SliceDeque`]? 

* Do you need to target OSes or targets without virtual-memory support? If so,
  [`SliceDeque`] is not an option, although [`VecDeque`] won't probably be an
  option either (these systems might lack an allocator). Still, getting
  [`VecDeque`] running on these systems is way easier than implementing support
  for [`SliceDeque`], which might not even be an option if the target does not
  have an Memory Management Unit.

* Is your deque small (smaller than the smallest capacity of a [`SliceDeque`])?
  If so, [`SliceDeque`] will unnecessarily use more memory than what you need.
  This might be an acceptable performance trade-off or not, depending on the
  application.

[`VecDeque`]: https://doc.rust-lang.org/std/collections/struct.VecDeque.html
[`as_slices`]: https://doc.rust-lang.org/std/collections/struct.VecDeque.html#method.as_slices
[`SliceDeque`]: struct.SliceDeque.html

[travis-shield]: https://img.shields.io/travis/gnzlbg/slice_deque.svg?style=flat-square
[travis]: https://travis-ci.org/gnzlbg/slice_deque
[appveyor-shield]: https://img.shields.io/appveyor/ci/gnzlbg/slice_deque.svg?style=flat-square
[appveyor]: https://ci.appveyor.com/project/gnzlbg/slice_deque/branch/master
[coveralls-shield]: https://img.shields.io/coveralls/gnzlbg/slice_deque.svg?style=flat-square
[coveralls]: https://coveralls.io/github/gnzlbg/slice_deque
[docs-shield]: https://img.shields.io/badge/docs-online-blue.svg?style=flat-square
[docs]: https://gnzlbg.github.io/slice_deque
[license-shield]: https://img.shields.io/badge/License-MIT%2FApache2.0-green.svg?style=flat-square
[license]: https://github.com/gnzlbg/slice_deque/blob/master/license.md
[crate-shield]: https://img.shields.io/crates/v/slice_deque.svg?style=flat-square
[crate]: https://crates.io/crates/slice_deque
