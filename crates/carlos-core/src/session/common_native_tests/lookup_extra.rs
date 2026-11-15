//! Exercise unscoped keys and named differences against the literal HEAD row algorithms.

use crate::backend::BackendKind;
use crate::session::models::MessageKind;
use crate::session::models::Role;
use crate::session::models::ToolOutputDetails;
use serde_json::json;

use super::head_reference::native::NativeUserHistory;
use super::head_reference::HeadReference;
use super::identity_support::{assert_entries, session_for as app, truncate};
use crate::session::item_history::replace_history_from_thread;
use crate::session::state::SessionState;

fn check(app: &SessionState, old: &HeadReference) {
    assert_entries(app);
    assert_eq!(app.messages().len(), old.messages.len());
    for (new, old) in app.messages().iter().zip(&old.messages) {
        assert_eq!(
            (new.role, new.kind, &new.text, &new.file_path),
            (old.role, old.kind, &old.text, &old.file_path)
        );
        assert_eq!(
            new.tool_output
                .as_ref()
                .map(|t| (&t.summary, t.generated, t.failed)),
            old.tool_output
                .as_ref()
                .map(|t| (&t.summary, t.generated, t.failed))
        );
    }
    for key in ["a", "b", "x#part-1"] {
        assert_eq!(
            app.item_message_index(&app.backend_entry_key(key)),
            old.item_message_index(key)
        );
    }
}

fn operate(app: &mut SessionState, old: &mut HeadReference, bits: u64) {
    let key = ["a", "b", "x#part-1"][(bits as usize / 17) % 3];
    match bits % 11 {
        0 => {
            app.ensure_item_placeholder(&app.backend_entry_key(key), Role::Assistant);
            old.ensure_item_placeholder(key, Role::Assistant);
        }
        1 => {
            app.upsert_agent_delta(&app.backend_entry_key(key), "delta");
            old.upsert_agent_delta(key, "delta");
        }
        2 => attach_tool(app, old, key, bits),
        3 => {
            app.set_command_override(&app.backend_entry_key(key), "native".into());
            old.set_command_override(key, "native".into());
        }
        4 => {
            app.upsert_mapped_message(
                &app.backend_entry_key(key),
                Role::ToolOutput,
                "+diff".into(),
                MessageKind::Diff,
                Some("b.rs".into()),
            );
            old.upsert_mapped_message(
                key,
                Role::ToolOutput,
                "+diff".into(),
                MessageKind::Diff,
                Some("b.rs".into()),
            );
        }
        5 => {
            app.mark_item_completed(&app.backend_entry_key(key));
            old.mark_item_completed(key);
        }
        6 => {
            let cut = bits as usize / 31 % (old.messages.len() + 1);
            truncate(app, cut);
            old.truncate(cut);
        }
        _ => failed_boundary(app, old, bits % 11),
    }
}

#[test]
fn unscoped_keys_preserve_heads_tool_facts_for_all_backends() {
    for backend in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        for seed in 1..=16 {
            let mut app = app(backend);
            let mut old = HeadReference::default();
            let mut bits = seed;
            for _ in 0..80 {
                bits ^= bits << 13;
                bits ^= bits >> 7;
                bits ^= bits << 17;
                operate(&mut app, &mut old, bits);
                check(&app, &old);
            }
        }
    }
}

#[test]
fn native_prompt_positions_match_the_unchanged_head_counter() {
    for count in 0..16 {
        let mut app = app(BackendKind::Codex);
        app.enable_native_history();
        let mut native = NativeUserHistory::default();
        let mut items = Vec::new();
        for n in 0..count {
            items.push(json!({"type":"userMessage","id":format!("hidden-{n}"),"content":[]}));
            native.record(&format!("hidden-{n}"), "turn", None);
        }
        items.push(json!({"type":"userMessage","id":"visible","content":[{"type":"text","text":"visible"}]}));
        let position = native.record("visible", "turn", Some(0));
        replace_history_from_thread(
            &mut app,
            &json!({"id":"thread","turns":[{"id":"turn","items":items}]}),
        )
        .expect("same-thread history");
        let entry = app.input_history().entry_id(0).expect("prompt entry");
        assert_eq!(
            app.input_history()
                .native_request(0, entry)
                .expect("native")
                .position,
            position
        );
        assert_eq!(
            app.messages()[0].kind,
            if position == 0 {
                MessageKind::Plain
            } else {
                MessageKind::UserSteer
            }
        );
        assert_eq!(native.message(0).expect("HEAD row").position, position);
    }
}

#[test]
fn named_exception_absent_diff_parts_retire_and_regrow_with_a_fresh_row() {
    // Entry identity and history rules require retirement. HEAD keeps the old
    // lookup and updates that old row on regrowth. Keep this difference explicit.
    let mut app = app(BackendKind::Codex);
    let mut head = HeadReference::default();
    for part in 0..3 {
        app.with_native_item_part("diff", part, |a, k| {
            a.upsert_mapped_message(
                &a.backend_entry_key(k),
                Role::ToolOutput,
                "+old".into(),
                MessageKind::Diff,
                None,
            );
        });
        head.with_item_part("diff", part, |a, k| {
            a.upsert_mapped_message(k, Role::ToolOutput, "+old".into(), MessageKind::Diff, None);
        });
    }
    let retired = app.messages()[2].id;
    app.transcript_retain_parts(&app.backend_entry_key("diff"), 1);
    app.with_native_item_part("diff", 2, |a, k| {
        assert_eq!(a.item_message_index(&a.backend_entry_key(k)), None);
        a.upsert_mapped_message(
            &a.backend_entry_key(k),
            Role::ToolOutput,
            "+new".into(),
            MessageKind::Diff,
            None,
        );
    });
    head.with_item_part("diff", 2, |a, k| {
        assert_eq!(a.item_message_index(k), Some(2));
        a.upsert_mapped_message(k, Role::ToolOutput, "+new".into(), MessageKind::Diff, None);
    });
    assert_eq!(head.messages.len(), 3);
    assert_eq!(app.messages().len(), 4);
    assert_eq!(app.messages()[2].text, "+old");
    assert_eq!(head.messages[2].text, "+new");
    assert_ne!(app.messages()[3].id, retired);
    assert_entries(&app);
}

fn attach_tool(app: &mut SessionState, old: &mut HeadReference, key: &str, bits: u64) {
    app.upsert_mapped_message(
        &app.backend_entry_key(key),
        Role::ToolOutput,
        "body".into(),
        MessageKind::Plain,
        Some("a.rs".into()),
    );
    old.upsert_mapped_message(
        key,
        Role::ToolOutput,
        "body".into(),
        MessageKind::Plain,
        Some("a.rs".into()),
    );
    let details = ToolOutputDetails {
        summary: "generated".into(),
        generated: true,
        failed: bits & 16 != 0,
    };
    app.transcript_set_tool(
        app.item_message_index(&app.backend_entry_key(key))
            .expect("row"),
        details.clone(),
    );
    let index = old.item_message_index(key).expect("row");
    old.messages[index].tool_output = Some(details);
}

fn failed_boundary(app: &mut SessionState, old: &mut HeadReference, operation: u64) {
    use super::forks::switch_session;

    use super::feed as handle_server_message_line;
    use crate::session::item_history::load_history_from_start_or_resume;

    if !app.claude().enabled() {
        let error = if operation & 1 == 0 {
            "not json"
        } else {
            r#"{"error":{"message":"rejected"}}"#
        };
        let result = if app.codex_scope().enabled() {
            app.adopt_codex_fork(error)
        } else {
            app.adopt_grok_fork(error, Vec::new())
        };
        assert!(result.is_err());
        return;
    }
    match operation {
        7 => {
            assert!(app
                .claude_history_control("sessions.info", &json!({"sessionId":"wrong"}))
                .is_err());
        }
        8 => {
            let mut update = Default::default();
            app.receive_claude_reset(
                &json!({"new_conversation_id":format!("reset-{}",app.messages().len())}),
                &mut update,
            );
            if app.messages().is_empty() {
                *old = HeadReference::default();
            }
        }
        9 => {
            let generation = app.claude().generation() + 1;
            switch_session(app, generation, "fork");
            handle_server_message_line(app,&json!({"method":"claude/sessionStartFailed","params":{"generation":generation,"message":"startup failed"}}).to_string());
            *old = HeadReference::default();
            // HEAD claude_notifications::startup_failure -> claude_edit::unavailable.
            old.append_message(
                Role::System,
                "Could not edit Claude message: startup failed. Draft kept.",
            );
        }
        _ => {
            assert!(load_history_from_start_or_resume(app, "not json").is_err());
        }
    }
}
