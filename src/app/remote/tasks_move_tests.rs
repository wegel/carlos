//! Compare the finite pending-arrow transform with ordinary sequential keyboard clamping.

use std::mem::size_of;
use std::time::Instant;

use super::{Move, MAX_TASK_ROWS};

fn expected(mut position: usize, count: usize, steps: &[isize]) -> usize {
    for step in steps {
        position = position.saturating_add_signed(*step).min(count - 1);
    }
    position
}

#[test]
fn every_small_key_sequence_matches_sequential_arrows_at_both_ends_and_all_fresh_counts() {
    for length in 0..=8 {
        for pattern in 0..(1usize << length) {
            let steps: Vec<_> = (0..length)
                .map(|bit| if pattern & (1 << bit) == 0 { -1 } else { 1 })
                .collect();
            let mut movement = Move::new();
            for step in &steps {
                movement.push(*step);
            }
            for count in 1..=8 {
                for position in 0..count {
                    assert_eq!(
                        movement.apply(position, count),
                        expected(position, count, &steps),
                        "count={count}, position={position}, steps={steps:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_same_pending_keys_preserve_order_after_count_changes_and_long_saturation() {
    for steps in [
        vec![-1, 1],
        vec![1, -1],
        vec![-8, 16, -8, 1],
        vec![8, -16, 8, -1],
        vec![isize::MAX, -1],
        vec![isize::MIN, 1],
    ] {
        let mut movement = Move::new();
        for step in &steps {
            movement.push(*step);
        }
        for count in [1, 2, 3, 8, 32, 768, MAX_TASK_ROWS] {
            for position in [0, count / 2, count - 1] {
                assert_eq!(
                    movement.apply(position, count),
                    expected(position, count, &steps)
                );
            }
        }
    }
}

#[test]
fn composing_keys_keeps_a_fixed_seventeen_kib_table() {
    let mut movement = Move::new();
    let capacity = movement.bounds.capacity();
    let started = Instant::now();
    for _ in 0..1000 {
        movement.push(-1);
        movement.push(1);
    }
    assert_eq!(movement.bounds.capacity(), capacity);
    assert_eq!(capacity * size_of::<[u16; 2]>(), 17408);
    eprintln!(
        "task arrow transform: {}bytes, 2000keys in {:?}",
        size_of::<Move>() + 17408,
        started.elapsed()
    );
}
