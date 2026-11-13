//! Apply session reports to local controls without interpreting backend JSON.

use carlos_core::session::actions::ServerRequestAction;
use carlos_core::session::backend_update::{BackendChange, BackendUpdate};

use super::state::AppState;

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
