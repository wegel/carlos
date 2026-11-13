//! Pass native metadata reads to the session owner.

use super::state::AppState;
use crate::backend::BackendClient;

/// Send at most one native metadata read at the existing timer point.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState) -> bool {
    crate::session::codex::reconnect_runtime::tick(client, &mut app.session)
}
