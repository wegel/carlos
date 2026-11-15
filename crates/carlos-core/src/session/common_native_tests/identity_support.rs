//! Keep native fixture cleanup and label lookup on the actual core data stores.

use super::feed as handle_server_message_line;
use crate::backend::BackendKind;
use crate::session::state::SessionState;
use serde_json::{json, Value};

pub(super) fn summary_token(session: &SessionState, item: &str) -> String {
    let key = session.backend_entry_key(item);
    let id = session
        .messages()
        .entry_id_for_item(&key)
        .expect("native row");
    session
        .summary_binding_for_test(id)
        .expect("summary binding")
}

pub(super) fn reset(session: &mut SessionState, root: &str) {
    session.set_thread_id(root.into());
    session.reset_for_forked_thread();
    session.reset_prompt_history();
}

pub(super) fn truncate(session: &mut SessionState, cut: usize) {
    session.truncate_at(cut);
    session.retain_summary_items();
    session.retain_prompt_entries();
}

pub(super) fn session_for(backend: BackendKind) -> SessionState {
    let mut app = SessionState::new("thread".into(), Default::default(), false);
    app.set_codex_enabled_for_test(backend == BackendKind::Codex);
    app.set_claude_enabled_for_test(backend == BackendKind::Claude);
    app.replace_codex_item_turn(Some("turn".into()));
    app.set_summary_enabled_for_test(true);
    app
}

pub(super) fn notify(app: &mut SessionState, method: &str, item: Value, source: Value) {
    let line = json!({"method":method,"params":{"threadId":app.thread_id(),"turnId":"turn",
        "item":item,"sourceKey":source}});
    handle_server_message_line(app, &line.to_string());
}

pub(super) fn assert_entries(session: &SessionState) {
    session.messages().assert_invariants();
    session
        .input_history()
        .assert_invariants(session.messages());
    session.assert_summary_invariants();
    for prompt in session.claude_history().unknown() {
        assert!(session.messages().entry(prompt.entry_id()).is_some());
    }
}
