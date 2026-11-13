//! Local editor effects returned after the core records conversation events.

/// Carry caller-local navigation changes, never transcript redraw hints.
#[derive(Default)]
pub struct ItemChanges {
    pub reset_navigation: bool,
    pub history_replaced: bool,
}
