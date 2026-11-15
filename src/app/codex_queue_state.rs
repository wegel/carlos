//! Keep queue focus and unsent text in the terminal that owns the editors.

#[derive(Default)]
pub(super) struct QueueView {
    pub(super) mode: bool,
    pub(super) show: bool,
    pub(super) selected: Option<String>,
    pub(super) recovery: Option<String>,
    pub(super) error: Option<String>,
}
