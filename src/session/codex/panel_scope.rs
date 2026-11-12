//! Pass connection identity and requested panel reads without retaining client focus.

use super::panel_state::PanelId;
use super::scope::CodexScope;

/// Borrow the client's explicit target for one synchronous session command.
#[derive(Clone, Copy)]
pub(crate) struct PanelScope<'a> {
    pub(crate) native: CodexScope,
    pub(crate) generation: u64,
    pub(crate) rebuilding: bool,
    pub(crate) visible: bool,
    pub(crate) detail: bool,
    pub(crate) selected: Option<&'a PanelId>,
}

/// Return only exact local selection/detail changes after a session command.
#[derive(Default)]
pub(crate) struct PanelUpdate {
    pub(crate) selected: Option<PanelId>,
    pub(crate) detail: bool,
    pub(crate) reset_scroll: bool,
}
