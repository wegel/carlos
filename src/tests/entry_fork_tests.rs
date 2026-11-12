//! Exercise each backend's real adoption point and failures before and after adoption.

use serde_json::json;

use super::{apply_forked_thread_history, apply_grok_rewound_history};
use crate::app::claude_session_history::switch_session;
use crate::app::notifications::handle_server_message_line;
use crate::app::state::AppState;
use crate::app::Role;
use crate::backend::BackendKind;

#[test]
fn fork_remints_prefix_and_retires_old_ids_for_all_backend_paths() {
    for backend in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        for length in 0..5 {
            let mut app = AppState::new("old".into());
            app.codex.enabled = backend == BackendKind::Codex;
            app.claude.enabled = backend == BackendKind::Claude;
            for _ in 0..length {
                app.append_message(Role::Assistant, "copied");
            }
            let prefix = app.messages.entries().to_vec();
            let old: Vec<_> = prefix.iter().map(|row| row.id).collect();
            let items: Vec<_> = (0..length)
                .map(|index| json!({"id":index.to_string(),"type":"agentMessage","text":"copied"}))
                .collect();
            let response =
                json!({"result":{"thread":{"id":"fork","turns":[{"id":"turn","items":items}]}}})
                    .to_string();
            match backend {
                BackendKind::Codex => {
                    apply_forked_thread_history(&mut app, &response).expect("fork")
                }
                BackendKind::Grok => apply_grok_rewound_history(
                    &mut app,
                    &response,
                    prefix.into_iter().map(Into::into).collect(),
                )
                .expect("fork"),
                BackendKind::Claude => {
                    switch_session(&mut app, 2, "fork");
                    for _ in 0..length {
                        app.append_message(Role::Assistant, "copied");
                    }
                }
            }
            assert_eq!(app.thread_id, "fork");
            assert_eq!(app.messages.len(), length);
            assert!(old.iter().all(|id| app.messages.entry(*id).is_none()));
            assert!(app.messages.iter().all(|row| !old.contains(&row.id)));
        }
    }
}

#[test]
fn failed_drafts_and_forks_keep_the_old_entries_and_native_bindings() {
    for response in ["not json", "{}", r#"{"error":{"message":"rejected"}}"#] {
        for grok in [false, true] {
            let mut app = AppState::new("old".into());
            app.ensure_item_placeholder("a", Role::Assistant);
            let id = app.messages[0].id;
            let result = if grok {
                apply_grok_rewound_history(&mut app, response, Vec::new())
            } else {
                apply_forked_thread_history(&mut app, response)
            };
            assert!(result.is_err());
            assert_eq!(app.thread_id, "old");
            assert_eq!(app.messages[0].id, id);
            assert_eq!(app.item_message_index("a"), Some(0));
        }
    }
}

#[test]
fn claude_start_failure_after_adoption_keeps_the_new_session_and_ids() {
    for length in 0..4 {
        let mut app = AppState::new("old".into());
        app.claude.enabled = true;
        let old = app.append_message(Role::Assistant, "old");
        let old = app.messages[old].id;
        switch_session(&mut app, 2, "fork");
        for _ in 0..length {
            app.append_message(Role::Assistant, "new");
        }
        let ids: Vec<_> = app.messages.iter().map(|row| row.id).collect();
        handle_server_message_line(
            &mut app,
            &json!({"method":"claude/sessionStartFailed",
            "params":{"generation":2,"message":"startup failed"}})
            .to_string(),
        );
        assert_eq!(app.thread_id, "fork");
        assert!(app.messages.entry(old).is_none());
        assert!(ids.iter().all(|id| app.messages.entry(*id).is_some()));
    }
}

impl AppState {
    pub(in crate::app) fn reject_fork_for_test(&mut self, response: &str, codex: bool) -> bool {
        if codex {
            apply_forked_thread_history(self, response).is_err()
        } else {
            apply_grok_rewound_history(self, response, Vec::new()).is_err()
        }
    }
}
