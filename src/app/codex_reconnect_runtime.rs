//! Pass native metadata reads to the session owner.

use super::state::AppState;
use carlos_core::backend::BackendClient;

/// Send at most one native metadata read at the existing timer point.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState) -> bool {
    carlos_core::session::codex::reconnect_runtime::tick(client, &mut app.session)
}
