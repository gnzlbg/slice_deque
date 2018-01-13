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

* constrained platform support: by necessity [`SliceDeque`] must use the
platform-specific virtual memory facilities of the underlying operating
system. While [`SliceDeque`] can work on all major operating systems,
currently only `MacOS X` and `Linux` are supported.

* no global allocator support: since the `Alloc`ator API does not support
virtual memory, to use platform-specific virtual memory support
[`SliceDeque`] must bypass the global allocator and talk directly to the
operating system. This can have negative performance consequences since
growing [`SliceDeque`] is always going to incur the cost of some system
calls.

* capacity constrained by virtual memory facilities: [`SliceDeque`] must
allocate two adjacent memory regions that map to the same region of physical
memory. Most operating systems allow this operation to be performed
exclusively on memory pages (or memory allocations that are multiples of a
memory page). As a consequence, the smalles [`SliceDeque`] that can be
created has typically a capacity of 2 memory pages, and it can grow only to
capacities that are a multiple of a memory page.

The main advantages of [`SliceDeque`] are:

* nicer API: since it `Deref`s to a slice, all operations that work on
slices are available for `SliceDeque`.

* efficient iteration: as efficient as for slices.

* simpler serialization: since one can just serialize/deserialize a single slice.

All in all, if your double-ended queues are small (smaller than a memory
page) or they get resized very often, `VecDeque` can perform better than
[`SliceDeque`]. Otherwise, [`SliceDeque`] typically performs better (see
the benchmarks), but platform support and global allocator bypass are two
reasons to weight in against its usage.

[`VecDeque`]: https://doc.rust-lang.org/std/collections/struct.VecDeque.html
[`as_slices`]: https://doc.rust-lang.org/std/collections/struct.VecDeque.html#method.as_slices
[`SliceDeque`]: struct.SliceDeque.html

[travis-shield]: https://img.shields.io/travis/gnzlbg/slice_deque.svg?style=flat-square
[travis]: https://travis-ci.org/gnzlbg/slice_deque
[appveyor-shield]: https://ci.appveyor.com/api/projects/status/do5lv0m61efb7wrb?svg=true
[appveyor]: https://ci.appveyor.com/project/gnzlbg/slice_deque/branch/master
[coveralls-shield]: https://img.shields.io/coveralls/gnzlbg/slice_deque.svg?style=flat-square
[coveralls]: https://coveralls.io/github/gnzlbg/slice_deque
[docs-shield]: https://img.shields.io/badge/docs-online-blue.svg?style=flat-square
[docs]: https://gnzlbg.github.io/slice_deque
[license-shield]: https://img.shields.io/github/license/mashape/apistatus.svg?style=flat-square
[license]: https://github.com/gnzlbg/slice_deque/blob/master/license.md
[crate-shield]: https://img.shields.io/crates/v/slice_deque.svg?style=flat-square
[crate]: https://crates.io/crates/slice_deque
