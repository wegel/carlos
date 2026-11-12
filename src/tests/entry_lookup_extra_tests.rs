//! Exercise unscoped keys and named differences against the literal HEAD row algorithms.

use serde_json::json;

use super::entry_identity_tests::app;
use super::head_entry_reference::native::NativeUserHistory;
use super::head_entry_reference::HeadReference;
use crate::app::item_history::replace_history_from_thread;
use crate::app::state::AppState;
use crate::app::{MessageKind, Role};
use crate::backend::BackendKind;
use crate::session::models::ToolOutputDetails;

fn check(app: &AppState, old: &HeadReference) {
    app.assert_entry_invariants();
    assert_eq!(app.session.messages().len(), old.messages.len());
    for (new, old) in app.session.messages().iter().zip(&old.messages) {
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
        assert_eq!(app.item_message_index(key), old.item_message_index(key));
    }
}

fn operate(app: &mut AppState, old: &mut HeadReference, bits: u64) {
    let key = ["a", "b", "x#part-1"][(bits as usize / 17) % 3];
    match bits % 11 {
        0 => {
            app.ensure_item_placeholder(key, Role::Assistant);
            old.ensure_item_placeholder(key, Role::Assistant);
        }
        1 => {
            app.upsert_agent_delta(key, "delta");
            old.upsert_agent_delta(key, "delta");
        }
        2 => attach_tool(app, old, key, bits),
        3 => {
            app.set_command_override(key, "native".into());
            old.set_command_override(key, "native".into());
        }
        4 => {
            app.upsert_mapped_message(
                key,
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
            app.mark_item_completed(key);
            old.mark_item_completed(key);
        }
        6 => {
            let cut = bits as usize / 31 % (old.messages.len() + 1);
            app.rewind_fork_from_message_idx(Some(cut));
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
        app.input_history.enable_native();
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
        app.set_rewind_selection_for_test(Some(0));
        assert_eq!(
            app.input_history
                .selected_native_request()
                .expect("native")
                .position,
            position
        );
        assert_eq!(
            app.session.messages()[0].kind,
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
        app.with_item_part("diff", part, |a, k| {
            a.upsert_mapped_message(k, Role::ToolOutput, "+old".into(), MessageKind::Diff, None);
        });
        head.with_item_part("diff", part, |a, k| {
            a.upsert_mapped_message(k, Role::ToolOutput, "+old".into(), MessageKind::Diff, None);
        });
    }
    let retired = app.session.messages()[2].id;
    app.session
        .transcript_retain_parts(&app.item_key("diff"), 1);
    app.with_item_part("diff", 2, |a, k| {
        assert_eq!(a.item_message_index(k), None);
        a.upsert_mapped_message(k, Role::ToolOutput, "+new".into(), MessageKind::Diff, None);
    });
    head.with_item_part("diff", 2, |a, k| {
        assert_eq!(a.item_message_index(k), Some(2));
        a.upsert_mapped_message(k, Role::ToolOutput, "+new".into(), MessageKind::Diff, None);
    });
    assert_eq!(head.messages.len(), 3);
    assert_eq!(app.session.messages().len(), 4);
    assert_eq!(app.session.messages()[2].text, "+old");
    assert_eq!(head.messages[2].text, "+new");
    assert_ne!(app.session.messages()[3].id, retired);
    app.assert_entry_invariants();
}

fn attach_tool(app: &mut AppState, old: &mut HeadReference, key: &str, bits: u64) {
    app.upsert_mapped_message(
        key,
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
    app.session
        .transcript_set_tool(app.item_message_index(key).expect("row"), details.clone());
    let index = old.item_message_index(key).expect("row");
    old.messages[index].tool_output = Some(details);
}

fn failed_boundary(app: &mut AppState, old: &mut HeadReference, operation: u64) {
    use crate::app::claude_session_history::{receive, switch_session};
    use crate::app::item_history::load_history_from_start_or_resume;
    use crate::app::notifications::handle_server_message_line;

    if !app.claude.enabled {
        let error = if operation & 1 == 0 {
            "not json"
        } else {
            r#"{"error":{"message":"rejected"}}"#
        };
        assert!(app.reject_fork_for_test(error, app.codex.enabled));
        return;
    }
    match operation {
        7 => {
            assert!(receive(app, "sessions.info", &json!({"sessionId":"wrong"})).is_err());
        }
        8 => {
            crate::app::claude_reset::receive(
                app,
                &json!({"new_conversation_id":format!("reset-{}",app.session.messages().len())}),
            );
            if app.session.messages().is_empty() {
                *old = HeadReference::default();
            }
        }
        9 => {
            let generation = app.claude.generation + 1;
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
