//! Apply the local editor effects of native queue replies.

use serde_json::Value;

use super::codex_queue::{apply, scope};
use super::state::AppState;
use crate::session::actions::ServerRequestAction;
use crate::session::codex::queue::QueueUpdate;
use crate::session::codex::queue_receive;

/// Preserve selection while the session invalidates a changed native queue.
pub(super) fn changed(app: &mut AppState, params: &Value) {
    let mut update = QueueUpdate::default();
    queue_receive::changed(&mut app.session, &mut update, params);
    apply(app, update);
}

/// Route exact-scope replies through the session and apply its local effects once.
pub(super) fn reply(app: &mut AppState, params: &Value) -> Option<ServerRequestAction> {
    let scope = scope(app);
    let mut update = QueueUpdate::default();
    let page = queue_receive::reply(&mut app.session, scope, &mut update, params);
    apply(app, update);
    page.map(|(token, cursor)| ServerRequestAction::CodexQueuePage { token, cursor })
}

/// Restore an unsent input without replacing text the user typed while awaiting a reply.
pub(super) fn recover(app: &mut AppState, text: Option<String>) {
    if let Some(text) = text {
        if app.input_is_empty() {
            app.set_input_text(&text);
        } else {
            app.view.codex_queue.recovery = Some(text);
        }
    }
}
