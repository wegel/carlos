//! Prove that the allocator refuses to wrap its checked counter.

use super::EntryIds;

#[test]
#[should_panic(expected = "entry id counter exhausted")]
fn exhausted_counter_cannot_wrap() {
    let mut ids = EntryIds::new();
    ids.next = u64::MAX;
    ids.allocate();
}
