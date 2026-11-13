//! Tracks exact Codex user-item positions without treating live activity as a steer.

use std::collections::HashMap;

use super::entry_id::EntryId;
#[cfg(any(test, feature = "test-support"))]
use super::transcript::Transcript;

/// Identifies one native user item and its position inside its enclosing turn.
#[derive(Clone)]
#[cfg_attr(any(test, feature = "test-support"), derive(Debug))]
pub struct NativeUserPosition {
    pub(in crate::session) item_id: String,
    pub(in crate::session) turn_id: String,
    pub(in crate::session) position: usize,
}

/// Records all user items, including those that have no visible text row.
#[derive(Default)]
#[cfg_attr(any(test, feature = "test-support"), derive(Debug))]
pub struct NativeUserHistory {
    positions: HashMap<(String, String), usize>,
    turn_counts: HashMap<String, usize>,
    messages: HashMap<EntryId, NativeUserPosition>,
}

impl NativeUserHistory {
    /// Deduplicate started/completed events by the exact native item and turn IDs.
    pub fn record(&mut self, item_id: &str, turn_id: &str, message: Option<EntryId>) -> usize {
        let key = (turn_id.to_string(), item_id.to_string());
        let position = *self.positions.entry(key).or_insert_with(|| {
            let count = self.turn_counts.entry(turn_id.to_string()).or_default();
            let position = *count;
            *count += 1;
            position
        });
        if let Some(message) = message {
            self.messages.insert(
                message,
                NativeUserPosition {
                    item_id: item_id.to_string(),
                    turn_id: turn_id.to_string(),
                    position,
                },
            );
        }
        position
    }

    /// Return the native identity attached to a visible user row.
    pub fn message(&self, index: EntryId) -> Option<&NativeUserPosition> {
        self.messages.get(&index)
    }
}

#[cfg(any(test, feature = "test-support"))]
impl NativeUserHistory {
    pub(crate) fn retain_entries(&mut self, transcript: &Transcript) {
        self.messages
            .retain(|id, _| transcript.entry(*id).is_some());
    }
}

#[cfg(any(test, feature = "test-support"))]
impl NativeUserHistory {
    /// Assert that the stored ids and their lookup maps still agree.
    pub fn assert_invariants(&self, transcript: &Transcript) {
        for (id, native) in &self.messages {
            assert!(transcript.entry(*id).is_some());
            assert_eq!(
                self.positions
                    .get(&(native.turn_id.clone(), native.item_id.clone())),
                Some(&native.position)
            );
            assert!(native.position < self.turn_counts[&native.turn_id]);
        }
    }
}
