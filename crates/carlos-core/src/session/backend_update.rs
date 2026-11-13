//! Return synchronous backend changes without introducing a second backend event protocol.

use super::claude::receive_update::ClaudeUpdate;
use super::codex::callback_update::CallbackUpdate;
use super::codex::frame_install::NativeInstalled;
use super::codex::lifecycle::CodexClosed;
use super::codex::panel_scope::PanelScope;
use super::common_update::CommonUpdate;
use super::item_changes::ItemChanges;

/// Carry native connection facts while finishing work already requested by a command.
pub struct BackendContext<'a> {
    pub panel: PanelScope<'a>,
}

/// Carry one existing handler report and a completed history fragment's nested report.
pub struct BackendUpdate {
    pub change: BackendChange,
    pub nested: Option<Box<BackendUpdate>>,
}

/// Distinguish the existing report families for immediate client application.
pub enum BackendChange {
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

impl super::state::SessionState {
    /// Finish requested reads using session identity, independent of any client's selected panel.
    pub fn backend_context(&self) -> BackendContext<'static> {
        BackendContext {
            panel: PanelScope {
                native: self.codex_scope(),
                generation: self.codex_connection().generation(),
                rebuilding: self.codex_connection().rebuilding(),
                visible: true,
                detail: false,
                selected: None,
            },
        }
    }
}
