//! Preserve the native selected-process guard until its task-owner successor lands.

use serde_json::json;

use super::codex_question_support::app;

#[test]
fn a_reused_process_id_with_a_different_native_item_never_retargets_stop() {
    let mut app = app();
    app.session.push_codex_process_for_test(
        carlos_core::session::codex::panel_state::Process::new(
            "child".into(),
            json!({"processId":"7","itemId":"old-item"}),
        ),
    );
    app.view.codex_panel.selected = app.session.codex_panel().ids().into_iter().next();
    app.session
        .set_codex_process_item_for_test(0, json!("new-item"));
    assert!(!app
        .session
        .codex_panel()
        .contains_target(app.view.codex_panel.selected.as_ref()));
    let peer = super::codex_question_support::Peer::default();
    crate::app::codex_panel::stop(&mut app);
    crate::app::codex_panel::tick(&peer, &mut app);
    assert!(peer.controls.borrow().is_empty());
    crate::app::codex_panel::select(&mut app, true);
    assert!(app
        .session
        .codex_panel()
        .contains_target(app.view.codex_panel.selected.as_ref()));
}
