//! Pass native metadata reads to the session owner.

use carlos_core::backend::BackendClient;

use super::state::AppState;

/// Send at most one native metadata read at the existing timer point.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState) -> bool {
    carlos_core::session::codex::reconnect_runtime::tick(client, &mut app.session)
}
