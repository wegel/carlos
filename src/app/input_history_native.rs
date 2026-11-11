//! Tracks exact Codex user-item positions without treating live activity as a steer.

use std::collections::HashMap;

/// Identifies one native user item and its position inside its enclosing turn.
#[derive(Clone)]
pub(super) struct NativeUserPosition {
    pub(super) item_id: String,
    pub(super) turn_id: String,
    pub(super) position: usize,
}

/// Records all user items, including those that have no visible text row.
#[derive(Default)]
pub(super) struct NativeUserHistory {
    positions: HashMap<(String, String), usize>,
    turn_counts: HashMap<String, usize>,
    messages: HashMap<usize, NativeUserPosition>,
}

impl NativeUserHistory {
    /// Deduplicate started/completed events by the exact native item and turn IDs.
    pub(super) fn record(&mut self, item_id: &str, turn_id: &str, message: Option<usize>) -> usize {
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
    pub(super) fn message(&self, index: usize) -> Option<&NativeUserPosition> {
        self.messages.get(&index)
    }
}
