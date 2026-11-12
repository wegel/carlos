//! Keep only local task-panel focus, navigation and detail visibility.

pub(super) use crate::session::codex::panel_state::PanelId;

/// Retain the terminal's selected native row independently of backend reads.
#[derive(Default)]
pub(super) struct PanelView {
    pub(super) show: bool,
    pub(super) selected: Option<PanelId>,
    pub(super) scroll: usize,
    pub(super) detail: bool,
}

impl PanelView {
    /// Keep selection stable; only explicit arrows may replace a vanished selected row.
    pub(super) fn select(
        &mut self,
        down: bool,
        facts: &crate::session::codex::panel_state::PanelState,
    ) {
        let ids = facts.ids();
        let current = self
            .selected
            .as_ref()
            .and_then(|id| ids.iter().position(|row| row == id));
        let next = current.map_or(0, |index| {
            if down {
                (index + 1).min(ids.len().saturating_sub(1))
            } else {
                index.saturating_sub(1)
            }
        });
        self.selected = ids.get(next).cloned();
        self.scroll = 0;
        self.detail = false;
    }
}
