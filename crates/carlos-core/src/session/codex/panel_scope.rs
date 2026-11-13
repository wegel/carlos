//! Pass connection identity and requested panel reads without retaining client focus.

use super::panel_state::PanelId;
use super::scope::CodexScope;

/// Borrow the client's explicit target for one synchronous session command.
#[derive(Clone, Copy)]
pub struct PanelScope<'a> {
    pub native: CodexScope,
    pub generation: u64,
    pub rebuilding: bool,
    pub visible: bool,
    pub detail: bool,
    pub selected: Option<&'a PanelId>,
}

/// Return only exact local selection/detail changes after a session command.
#[derive(Default)]
pub struct PanelUpdate {
    pub selected: Option<PanelId>,
    pub detail: bool,
    pub reset_scroll: bool,
}
