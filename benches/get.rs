#![feature(test)]

extern crate test;
extern crate virtual_deque;

use std::collections::VecDeque;

const MAX_IDX: usize = 100_000;

#[bench]
fn get_contiguous_std_vecdeque(b: &mut test::Bencher) {
    let mut deq = VecDeque::<u8>::with_capacity(MAX_IDX);
    deq.resize(MAX_IDX, 3);
    b.iter(|| {
        for i in 0..MAX_IDX {
            test::black_box(&deq[i]);
        }
    });
}

#[bench]
fn get_contiguous_virtual_deque(b: &mut test::Bencher) {
    let mut deq = virtual_deque::VirtualDeque::<u8>::with_capacity(MAX_IDX);
    deq.resize(MAX_IDX, 3);
    b.iter(|| {
        for i in 0..MAX_IDX {
            test::black_box(&deq[i]);
        }
    });
}

#[bench]
fn get_chunked_std_vecdeque(b: &mut test::Bencher) {
    let mut deq = VecDeque::<u8>::with_capacity(MAX_IDX);
    deq.resize(MAX_IDX, 3);
    for _ in 0..MAX_IDX / 2 {
        deq.pop_front();
    }
    for _ in 0..MAX_IDX / 4 {
        deq.push_back(3);
    }
    b.iter(|| {
        for i in 0..MAX_IDX / 4 * 3 {
            test::black_box(&deq[i]);
        }
    });
}

#[bench]
fn get_chunked_virtual_deque(b: &mut test::Bencher) {
    let mut deq = virtual_deque::VirtualDeque::<u8>::with_capacity(MAX_IDX);
    deq.resize(MAX_IDX, 3);
    for _ in 0..MAX_IDX / 2 {
        deq.pop_front();
    }
    for _ in 0..MAX_IDX / 4 {
        deq.push_back(3);
    }
    b.iter(|| {
        for i in 0..MAX_IDX / 4 * 3 {
            test::black_box(&deq[i]);
        }
    });
}
