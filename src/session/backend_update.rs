//! Return synchronous backend changes without introducing a second backend event protocol.

use super::claude::receive_update::{ClaudeDraftContext, ClaudeUpdate};
use super::codex::callback_update::CallbackUpdate;
use super::codex::frame_install::NativeInstalled;
use super::codex::lifecycle::CodexClosed;
use super::codex::panel_scope::PanelScope;
use super::common_update::CommonUpdate;
use super::item_changes::ItemChanges;

/// Borrow explicit client targets and unsent text for one immediate session call.
pub(crate) struct BackendContext<'a> {
    pub(crate) panel: PanelScope<'a>,
    pub(crate) callback_focused: bool,
    pub(crate) claude: ClaudeDraftContext<'a>,
}

/// Carry one existing handler report and a completed history fragment's nested report.
pub(crate) struct BackendUpdate {
    pub(crate) change: BackendChange,
    pub(crate) nested: Option<Box<BackendUpdate>>,
}

/// Distinguish the existing report families for immediate client application.
pub(crate) enum BackendChange {
    None,
    Items(ItemChanges),
    Installed(NativeInstalled),
    Common(CommonUpdate),
    Codex(CallbackUpdate),
    Claude(ClaudeUpdate),
    Closed(CodexClosed),
}

impl BackendUpdate {
    /// Return a handler report without an assembled history fragment.
    pub(in crate::session) fn new(change: BackendChange) -> Self {
        Self {
            change,
            nested: None,
        }
    }
}
