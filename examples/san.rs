//! Tests that run under the sanitizers.
#![cfg_attr(
    feature = "cargo-clippy",
    allow(cyclomatic_complexity, option_unwrap_used)
)]

#[macro_use]
extern crate slice_deque;

use slice_deque::SliceDeque;

fn main() {
    // Single-threaded stable no-std tests:
    {
        let mut deq: SliceDeque<u32> = SliceDeque::with_capacity(10);
        assert!(deq.capacity() >= 10);
        assert!(!deq.is_full());
        assert!(deq.is_empty());
        assert!(deq.len() == 0);
        deq.push_back(3);
        assert!(deq.len() == 1);
        assert!(!deq.is_full());
        assert!(!deq.is_empty());

        let mut deq2 = sdeq![4, 5, 6];
        deq.append(&mut deq2);
        assert_eq!(deq, [3, 4, 5, 6]);
        assert_eq!(deq2, []);

        assert_eq!(deq.front(), Some(&3));
        deq.push_front(3);
        assert_eq!(deq.front(), Some(&3));
        (*deq.front_mut().unwrap()) = 2;
        assert_eq!(deq.front(), Some(&2));

        assert_eq!(deq.back(), Some(&6));
        deq.push_front(3);
        assert_eq!(deq.back(), Some(&6));
        (*deq.back_mut().unwrap()) = 3;
        assert_eq!(deq.back(), Some(&3));

        assert_eq!(deq.pop_front(), Some(3));
        assert_eq!(deq.pop_front(), Some(2));
        assert_eq!(deq.pop_front(), Some(3));

        deq.push_back(7);
        assert_eq!(deq, [4, 5, 3, 7]);

        let cap = deq.capacity();
        while cap == deq.capacity() {
            deq.push_back(1);
        }
        assert!(deq != [4, 5, 3, 7]);
        assert!(deq.capacity() > cap);
        while deq != [4, 5, 3, 7] {
            deq.pop_back();
        }
        assert_eq!(deq, [4, 5, 3, 7]);

        deq.shrink_to_fit();
        assert_eq!(deq.capacity(), cap);
    }
}
