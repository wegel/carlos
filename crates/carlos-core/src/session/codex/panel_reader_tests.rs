//! Keep a native task read with its captured root even when a caller changes the current thread.

use serde_json::json;

use super::panel;
use super::panel_pages;
use super::panel_scope::PanelScope;
use super::scope::CodexScope;
use crate::session::state::SessionState;

#[test]
fn task_pages_never_follow_a_replacement_root_or_publish_old_rows_into_it() {
    let mut session = SessionState::new("original".into(), Default::default(), false);
    session.codex_panel.agents = vec![json!({"id":"previous accepted row"})];
    let scope = PanelScope {
        native: CodexScope::for_test(true, false, 7),
        generation: 1,
        rebuilding: false,
        visible: false,
        detail: false,
        selected: None,
    };
    panel::refresh(&mut session, scope, false);
    session.thread_id = "replacement".into();
    panel_pages::page(
        &mut session,
        scope,
        &mut Default::default(),
        &json!({"data":[],"nextCursor":null}),
    )
    .expect("read retains its own target");
    let read = session
        .codex_panel
        .snapshot
        .as_ref()
        .expect("background process read");
    assert_eq!(read.request.method, "thread/backgroundTerminals/list");
    assert_eq!(read.request.params["threadId"], "original");
    panel_pages::page(
        &mut session,
        scope,
        &mut Default::default(),
        &json!({"data":[],"nextCursor":null}),
    )
    .expect("stale completed window is discarded");
    assert_eq!(
        session.codex_panel.agents,
        vec![json!({"id":"previous accepted row"})]
    );
    assert!(session
        .codex_panel
        .error
        .as_deref()
        .expect("stale root diagnostic")
        .contains("retired root"));
}
