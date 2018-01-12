#![feature(test)]

extern crate test;
extern crate virtual_deque;

use std::collections::VecDeque;

const MAX_NO_ITERS: usize = 10_000_000_000;

#[bench]
fn pop_front_std_vecdeque(b: &mut test::Bencher) {
    let mut deq = VecDeque::<u8>::with_capacity(MAX_NO_ITERS);
    deq.resize(MAX_NO_ITERS, 3);
    b.iter(|| {
        test::black_box(deq.pop_front().unwrap());
    });
}

#[bench]
fn pop_front_virtual_deque(b: &mut test::Bencher) {
    let mut deq = virtual_deque::VirtualDeque::<u8>::with_capacity(MAX_NO_ITERS);
    deq.resize(MAX_NO_ITERS, 3);
    b.iter(|| {
        test::black_box(deq.pop_front().unwrap());
    });
}
