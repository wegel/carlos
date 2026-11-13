//! Pass native metadata reads to the session owner.

use serde_json::Value;

use super::state::AppState;
use crate::backend::BackendClient;

/// Send at most one native metadata read at the existing timer point.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState) -> bool {
    crate::session::codex::reconnect_runtime::tick(client, &mut app.session)
}
/// Apply only the native metadata reply that matches the active reader.
pub(super) fn reply(app: &mut AppState, params: &Value) -> bool {
    crate::session::codex::reconnect_runtime::reply(&mut app.session, params)
}
