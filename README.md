# Slice Ring Buffer

[![maintenance](https://img.shields.io/badge/maintenance-passively--maintained-orange)](https://img.shields.io/badge/maintenance-passively--maintained-orange)
[![Rust](https://github.com/LiquidityC/slice_ring_buffer/actions/workflows/rust.yml/badge.svg)](https://github.com/LiquidityC/slice_ring_buffer/actions/workflows/rust.yml)
[Coverage Status](https://coveralls.io/repos/github/LiquidityC/slice_ring_buffer/badge.svg?branch=master)](h

> A double-ended queue that `Deref`s into a slice, also known as a ring buffer or circular buffer.

## :warning: Notice :warning:

***This is a fork of [SliceDequeue](https://github.com/gnzlbg/slice_deque) that
adds a security patch due to the original project no longer being maintained.
Contributions are welcome but there will be no active development on the
project by me.***

## Advantages

The main advantages of [`SliceRingBuffer`] are:

* nicer API: since it `Deref`s to a slice, all operations that work on
slices (like `sort`) "just work" for `SliceRingBuffer`.

* efficient: as efficient as a slice (iteration, sorting, etc.), more efficient
  in general than `VecDeque`.

## Platform Support

Windows, Linux, MacOS and every other unix-like OS is supported (although maybe
untested). The following targets are known to work and pass all tests:

### Linux

* aarch64-unknown-linux-gnu
* arm-unknown-linux-gnueabi
* arm-unknown-linux-musleabi
* armv7-unknown-linux-gnueabihf
* armv7-unknown-linux-musleabihf
* i586-unknown-linux-gnu
* i686-unknown-linux-gnu
* i686-unknown-linux-musl
* mips-unknown-linux-gnu
* mips64-unknown-linux-gnuabi64
* mips64el-unknown-linux-gnuabi64
* mipsel-unknown-linux-gnu
* powerpc-unknown-linux-gnu
* powerpc64-unknown-linux-gnu
* powerpc64le-unknown-linux-gnu
* x86_64-unknown-linux-gnu
* x86_64-unknown-linux-musl
* aarch64-linux-android
* arm-linux-androideabi
* armv7-linux-androideabi
* x86_64-linux-android

### MacOS X

* i686-apple-darwin
* x86_64-apple-darwin

### Windows

* x86_64-pc-windows-msvc

## Drawbacks

The main drawbacks of [`SliceRingBuffer`] are:

* "constrained" platform support: the operating system must support virtual
  memory. In general, if you can use `std`, you can use [`SliceRingBuffer`].

* global allocator bypass: [`SliceRingBuffer`] bypasses Rust's global allocator / it
  is its own memory allocator, talking directly to the OS. That is, allocating
  and growing [`SliceRingBuffer`]s always involve system calls, while a [`VecDeque`]
  backed-up by a global allocator might receive memory owned by the allocator
  without any system calls at all.
  
* smallest capacity constrained by the allocation granularity of the OS: some operating systems 
  allow [`SliceRingBuffer`] to allocate memory in 4/8/64 kB chunks. 

When shouldn't you use it? In my opinion, if

* you need to target `#[no_std]`, or
* you can't use it (because your platform doesn't support it)

you must use something else. If.

* your ring-buffer's are very small,

then by using [`SliceRingBuffer`] you might be trading memory for performance. Also,

* your application has many short-lived ring-buffers,

the cost of the system calls required to set up and grow the [`SliceRingBuffer`]s
might not be amortized by your application (update: there is a pull-request open
that caches allocations in thread-local heaps when the feature `use_std` is
enabled significantly improving the performance of short-lived ring-buffers, but
it has not been merged yet). Whether any of these trade-offs are worth it or not
is application dependent, so don't take my word for it: measure.

## How it works

The double-ended queue in the standard library ([`VecDeque`]) is implemented
using a growable ring buffer (`0` represents uninitialized memory, and `T`
represents one element in the queue):

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

This crates provides [`SliceRingBuffer`], a double-ended queue implemented with
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
`SliceRingBuffer` maps the physical memory to two adjacent memory regions, in virtual
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
Unit (MMU), [`SliceRingBuffer`] is able to `Deref`s into a slice in those systems.

This simplifies [`SliceRingBuffer`]'s API and implementation, giving it a performance
advantage over [`VecDeque`] in some situations. 

In general, you can think of [`SliceRingBuffer`] as a `Vec` with `O(1)` `pop_front`
and amortized `O(1)` `push_front` methods.

## License

This project is licensed under either of

* Apache License, Version 2.0, (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in SliceRingBuffer by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[`VecDeque`]: https://doc.rust-lang.org/std/collections/struct.VecDeque.html
[`as_slices`]: https://doc.rust-lang.org/std/collections/struct.VecDeque.html#method.as_slices
[`SliceRingBuffer`]: struct.SliceRingBuffer.html

[travis-shield]: https://img.shields.io/travis/gnzlbg/slice_deque.svg?style=flat-square
[travis]: https://travis-ci.org/gnzlbg/slice_deque
[appveyor-shield]: https://img.shields.io/appveyor/ci/gnzlbg/slice-deque.svg?style=flat-square
[appveyor]: https://ci.appveyor.com/project/gnzlbg/slice-deque/branch/master
[codecov-shield]: https://img.shields.io/codecov/c/github/gnzlbg/slice_deque.svg?style=flat-square
[codecov]: https://codecov.io/gh/gnzlbg/slice_deque
[docs-shield]: https://img.shields.io/badge/docs-online-blue.svg?style=flat-square
[docs]: https://docs.rs/crate/slice-deque/
[license-shield]: https://img.shields.io/badge/License-MIT%2FApache2.0-green.svg?style=flat-square
[license]: https://github.com/gnzlbg/slice_deque/blob/master/license.md
[crate-shield]: https://img.shields.io/crates/v/slice_deque.svg?style=flat-square
[crate]: https://crates.io/crates/slice_deque
