//! Exercise each backend's real adoption point and failures before and after adoption.

use crate::backend::BackendKind;
use crate::session::models::Role;
use serde_json::json;

use super::feed as handle_server_message_line;
use super::session;
use crate::session::state::SessionState;

#[test]
fn fork_remints_prefix_and_retires_old_ids_for_all_backend_paths() {
    for backend in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        for length in 0..5 {
            let mut app = session("old");
            app.set_codex_enabled_for_test(backend == BackendKind::Codex);
            app.set_claude_enabled_for_test(backend == BackendKind::Claude);
            for _ in 0..length {
                app.append_message(Role::Assistant, "copied");
            }
            let prefix = app.messages().entries().to_vec();
            let old: Vec<_> = prefix.iter().map(|row| row.id).collect();
            let items: Vec<_> = (0..length)
                .map(|index| json!({"id":index.to_string(),"type":"agentMessage","text":"copied"}))
                .collect();
            let response =
                json!({"result":{"thread":{"id":"fork","turns":[{"id":"turn","items":items}]}}})
                    .to_string();
            match backend {
                BackendKind::Codex => {
                    app.adopt_codex_fork(&response).expect("fork");
                }
                BackendKind::Grok => {
                    app.adopt_grok_fork(&response, prefix.into_iter().map(Into::into).collect())
                        .expect("fork");
                }
                BackendKind::Claude => {
                    switch_session(&mut app, 2, "fork");
                    for _ in 0..length {
                        app.append_message(Role::Assistant, "copied");
                    }
                }
            }
            assert_eq!(app.thread_id(), "fork");
            assert_eq!(app.messages().len(), length);
            assert!(old.iter().all(|id| app.messages().entry(*id).is_none()));
            assert!(app.messages().iter().all(|row| !old.contains(&row.id)));
        }
    }
}

#[test]
fn failed_drafts_and_forks_keep_the_old_entries_and_native_bindings() {
    for response in ["not json", "{}", r#"{"error":{"message":"rejected"}}"#] {
        for grok in [false, true] {
            let mut app = session("old");
            app.ensure_item_placeholder(&app.backend_entry_key("a"), Role::Assistant);
            let id = app.messages()[0].id;
            let result = if grok {
                app.adopt_grok_fork(response, Vec::new())
            } else {
                app.adopt_codex_fork(response)
            };
            assert!(result.is_err());
            assert_eq!(app.thread_id(), "old");
            assert_eq!(app.messages()[0].id, id);
            assert_eq!(app.item_message_index(&app.backend_entry_key("a")), Some(0));
        }
    }
}

#[test]
fn claude_start_failure_after_adoption_keeps_the_new_session_and_ids() {
    for length in 0..4 {
        let mut app = session("old");
        app.set_claude_enabled_for_test(true);
        let old = app.append_message(Role::Assistant, "old");
        let old = app.messages()[old].id;
        switch_session(&mut app, 2, "fork");
        for _ in 0..length {
            app.append_message(Role::Assistant, "new");
        }
        let ids: Vec<_> = app.messages().iter().map(|row| row.id).collect();
        handle_server_message_line(
            &mut app,
            &json!({"method":"claude/sessionStartFailed",
            "params":{"generation":2,"message":"startup failed"}})
            .to_string(),
        );
        assert_eq!(app.thread_id(), "fork");
        assert!(app.messages().entry(old).is_none());
        assert!(ids.iter().all(|id| app.messages().entry(*id).is_some()));
    }
}

pub(super) fn switch_session(session: &mut SessionState, generation: u64, id: &str) {
    let mut update = Default::default();
    session.switch_claude_session(generation, id, &mut update);
    session.publish();
}
