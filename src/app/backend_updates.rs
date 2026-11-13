//! Apply session reports to local controls without interpreting backend JSON.

use carlos_core::session::actions::ServerRequestAction;
use carlos_core::session::backend_update::{BackendChange, BackendContext, BackendUpdate};
use carlos_core::session::claude::receive_update::ClaudeDraftContext;

use super::state::AppState;

/// Borrow local targets independently from the mutable session owner.
pub(super) fn context<'a>(
    session: &carlos_core::session::state::SessionState,
    view: &'a super::view_state::ViewState,
) -> BackendContext<'a> {
    BackendContext {
        panel: super::codex_panel::scope(
            session.codex_scope(),
            session.codex_connection(),
            &view.codex_panel,
        ),
        callback_focused: view.codex_callbacks.focused,
        claude: ClaudeDraftContext {
            composer: view.input.lines(),
            confirmation: view.confirm_allow,
        },
    }
}

impl AppState {
    /// Apply the parent before its assembled history fact, discarding the nested action as before.
    pub(super) fn apply_backend_update(
        &mut self,
        update: BackendUpdate,
    ) -> Option<ServerRequestAction> {
        let action = match update.change {
            BackendChange::None => None,
            BackendChange::Items(changes) => {
                self.apply_item_changes(changes);
                None
            }
            BackendChange::Installed(installed) => {
                self.apply_native_install(installed);
                None
            }
            BackendChange::Common(update) => self.apply_common_update(update),
            BackendChange::Codex(update) => super::codex_callbacks::apply(self, update),
            BackendChange::Claude(update) => self.apply_claude_update(update),
            BackendChange::Closed(closed) => {
                self.apply_codex_closed(closed);
                None
            }
        };
        if let Some(nested) = update.nested {
            let _ = self.apply_backend_update(*nested);
        }
        action
    }
}
