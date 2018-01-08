# Virtual Deque

> A double-ended queue implemented with a growable virtual ring buffer.

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

As a consequence, [`VecDeque`] cannot `Deref` into a slice, since its
elements do not, in general, occupy a contiguous memory region. This
complicates the implementation and its interface (for example, there is no
`as_slice` method, but [`as_slices`] returns a pair of slices) and has
negative performance consequences (e.g. need to account for wrap around
while iterating over the elements).

This crates provides [`VirtualDeque`], a double-ended queue implemented with
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

That is, both the virtual memory regions `0` and `1` above (top) map to the
same physical memory (bottom). Such that when the queue grows beyond the end
of the allocated buffer growing into the adjacent virtual memory region is
equivalent to wrapping around the physical memory:

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

As a consequence, [`VirtualDeque`] `Deref`s into a slice, simplifying its
API and implementation, and leading to better performance in some situations.

The main drawbacks of [`VirtualDeque`] are:

* constrained platform support: by necessity [`VirtualDeque`] must use the
platform-specific virtual memory facilities of the underlying operating
system. While [`VirtualDeque`] can work on all major operating systems,
currently only `MacOS X` is supported.

* no global allocator support: since the `Alloc`ator API does not support
virtual memory, to use platform-specific virtual memory support
[`VirtualDeque`] must bypass the global allocator and talk directly to the
operating system. This can have negative performance consequences since
growing [`VirtualDeque`] is always going to incur the cost of some system
calls.

* capacity constrained by virtual memory facilities: [`VirtualDeque`] must
allocate two adjacent memory regions that map to the same region of physical
memory. Most operating systems allow this operation to be performed
exclusively on memory pages (or memory allocations that are multiples of a
memory page). As a consequence, the smalles [`VirtualDeque`] that can be
created has typically a capacity of 2 memory pages, and it can grow only to
capacities that are a multiple of a memory page.

The main advantages of [`VirtualDeque`] are:

* nicer API: since it `Deref`s to a slice, all operations that work on
slices are available for `VirtualDeque`.

* efficient iteration: as efficient as for slices.

* simpler serialization: since one can just serialize/deserialize a single slice.

All in all, if your double-ended queues are small (smaller than a memory
page) or they get resized very often, `VecDeque` can perform better than
[`VirtualDeque`]. Otherwise, [`VirtualDeque`] typically performs better (see
the benchmarks), but platform support and global allocator bypass are two
reasons to weight in against its usage.

[`VecDeque`]: https://doc.rust-lang.org/std/collections/struct.VecDeque.html
[`as_slices`]: https://doc.rust-lang.org/std/collections/struct.VecDeque.html#method.as_slices
[`VirtualDeque`]: struct.VirtualDeque.html
