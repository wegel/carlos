//! Preserve native history paging, hidden metadata, source rows, and partial failure behavior.

use crate::app::normalized_test_app as normalized_app;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};

use self::normalized_app::{handle_server_message_line, NormalizedApp};
use carlos_core::session::actions::ServerRequestAction;
use carlos_core::session::claude::ClaudePhase;

fn native(app: &mut NormalizedApp, request: &str, result: Value) -> Option<ServerRequestAction> {
    handle_server_message_line(
        app,
        &json!({"method":"claude/controller","params":{
        "generation":1,"requestMethod":request,"frame":{"type":"reply","ok":true,"result":result}}})
        .to_string(),
    )
}

fn record(session: &str, uuid: &str, source: &str, text: &str) -> Value {
    json!({"source":source,"message":{"type":"user","session_id":session,"uuid":uuid,
        "opaque":{"kept":true},"message":{"role":"user","content":text}}})
}

fn control(action: Option<ServerRequestAction>, expected: &str, params: Value) {
    match action.expect("native control") {
        ServerRequestAction::ClaudeControl {
            method,
            params: actual,
        } => {
            assert_eq!(method, expected);
            assert_eq!(actual, params);
        }
        _ => panic!("expected Claude control"),
    }
}

#[test]
fn foreign_record_fails_after_retaining_earlier_rows_and_metadata() {
    let mut app = NormalizedApp::new("session".into());
    native(
        &mut app,
        "sessions.read",
        json!({"messages":[
        record("session","first","human","earlier"),record("other","wrong","human","foreign")]}),
    );
    assert_eq!(app.session.claude().phase(), ClaudePhase::Closed);
    assert_eq!(app.session.thread_id(), "session");
    assert_eq!(app.session.claude_history().records().len(), 1);
    assert_eq!(app.session.messages()[0].text, "earlier");
    assert_eq!(
        app.session.messages()[1].text,
        "Claude history belongs to a different session"
    );
    assert_eq!(app.input_history_len(), 1);
    assert_eq!(
        app.session.claude_history().records()[0]["opaque"]["kept"],
        true
    );
}

#[test]
fn child_records_keep_metadata_but_hide_prompt_and_transcript_rows() {
    let mut app = NormalizedApp::new("session".into());
    let mut child = record("session", "child", "human", "hidden");
    child["message"]["parent_tool_use_id"] = json!("task");
    native(
        &mut app,
        "sessions.read",
        json!({"messages":[child,
        record("session","visible","unknown","legacy")],"nextOffset":null}),
    );
    assert_eq!(app.session.claude_history().records().len(), 2);
    assert_eq!(
        app.session.claude_history().records()[0]["opaque"]["kept"],
        true
    );
    assert_eq!(app.session.messages().len(), 1);
    assert_eq!(app.session.messages()[0].text, "[Source unknown] legacy");
    assert_eq!(app.session.claude_history().unknown().len(), 1);
    assert_eq!(app.session.claude_history().unknown()[0].uuid(), "visible");
    assert_eq!(app.input_history_len(), 0);
}

#[test]
fn native_chunks_reject_wrong_offset_oversized_total_and_declared_overrun() {
    for (offset, total, bytes) in [(1, 2, "x"), (0, 16 * 1024 * 1024 + 1, "x"), (0, 1, "xx")] {
        let mut app = NormalizedApp::new("session".into());
        native(
            &mut app,
            "sessions.read",
            json!({"messages":[record("session","first","human","earlier")],"nextOffset":1}),
        );
        native(
            &mut app,
            "sessions.read",
            json!({"chunk":{"offset":offset,"totalBytes":total,"data":STANDARD.encode(bytes)}}),
        );
        assert_eq!(app.session.claude().phase(), ClaudePhase::Closed);
        assert_eq!(app.session.claude_history().records().len(), 1);
        assert_eq!(app.session.messages()[0].text, "earlier");
        assert_eq!(
            app.session.messages()[1].text,
            "Claude history chunk exceeds its declared size or arrived out of order"
        );
    }
}

#[test]
fn history_controls_keep_snapshot_fields_and_default_chunk_offset() {
    let mut app = super::state::AppState::new("session".into());
    for (offset, expected) in [(None, 0), (Some(json!(7)), 7), (Some(json!("bad")), 0)] {
        let mut result = json!({"nextOffset":9,"snapshotId":{"opaque":true}});
        if let Some(offset) = offset {
            result["chunkOffset"] = offset;
        }
        control(
            super::claude_session_history::receive(&mut app, "sessions.read", &result).ok(),
            "sessions.read",
            json!({"sessionId":"session","offset":9,"snapshotId":{"opaque":true},"chunkOffset":expected}),
        );
    }
    for id in [Value::Null, json!(7), json!("foreign")] {
        assert!(super::claude_session_history::receive(
            &mut app,
            "sessions.info",
            &json!({"sessionId":id})
        )
        .is_err());
    }
    control(
        super::claude_session_history::receive(
            &mut app,
            "sessions.info",
            &json!({"sessionId":"session","customTitle":"  named  "}),
        )
        .ok(),
        "sessions.read",
        json!({"sessionId":"session"}),
    );
    assert_eq!(app.session.thread_name().as_deref(), Some("named"));
    assert_eq!(app.session.status(), "Reading Claude history");
    control(
        super::claude_session_history::receive(&mut app, "sessions.read", &json!({})).ok(),
        "start",
        json!({"sessionId":"session"}),
    );
}

#[test]
fn normalized_parts_reject_bounds_without_erasing_earlier_rows() {
    for params in [
        json!({"offset":1,"totalBytes":2,"text":"x"}),
        json!({"offset":0,"totalBytes":67108865,"text":"x"}),
        json!({"offset":0,"totalBytes":1,"text":"xx"}),
        json!({"totalBytes":2,"text":"x"}),
        json!({"offset":0,"text":"x"}),
        json!({"offset":0,"totalBytes":2}),
        json!({"offset":0,"totalBytes":1,"text":"!"}),
    ] {
        let mut app = super::state::AppState::new("session".into());
        app.append_message(carlos_core::session::models::Role::Assistant, "earlier");
        let mut params = params;
        params["generation"] = json!(1);
        params["epoch"] = json!(0);
        assert!(super::claude_notifications::handle(
            &mut app,
            "claude/historyFactPart",
            params.as_object().expect("params")
        )
        .is_none());
        assert_eq!(app.session.claude().phase(), ClaudePhase::Closed);
        assert_eq!(app.session.messages()[0].text, "earlier");
        assert!(app.session.messages().len() >= 2);
    }
}

#[test]
fn normalized_completion_routes_again_and_acknowledges_even_when_inner_fact_is_dropped() {
    for epoch in [0, 4] {
        let mut app = super::state::AppState::new("session".into());
        super::claude_notifications::handle(
            &mut app,
            "claude/conversationReset",
            json!({"generation":1,"epoch":4,"new_conversation_id":"reset"})
                .as_object()
                .expect("params"),
        );
        let text = json!({"method":"claude/historyPrompt","params":{
            "generation":999,"epoch":epoch,"source":"human","uuid":"prompt","text":"row"}})
        .to_string();
        let split = text.len() / 2;
        for (offset, piece) in [(0, &text[..split]), (split, &text[split..])] {
            let params = json!({"generation":1,"epoch":4,"offset":offset,"totalBytes":text.len(),"text":piece});
            control(
                super::claude_notifications::handle(
                    &mut app,
                    "claude/historyFactPart",
                    params.as_object().expect("params"),
                ),
                "history.continue",
                json!({"generation":1,"epoch":4}),
            );
        }
        assert_eq!(app.session.messages().len(), usize::from(epoch == 4));
        if epoch == 4 {
            assert_eq!(app.session.messages()[0].text, "row");
        }
    }
}

#[test]
fn source_defaults_duplicates_and_empty_uuids_keep_their_old_rows() {
    let mut app = super::state::AppState::new("session".into());
    app.session.enable_claude();
    for facts in [
        json!({"uuid":"same","source":"human","text":"human","inputKind":"steer"}),
        json!({"uuid":"same","source":"automatic","text":"duplicate"}),
        json!({"uuid":"legacy","text":"old"}),
        json!({"source":"automatic","text":"automatic"}),
        json!({"source":"system","uuid":"system","text":"hidden"}),
        json!({}),
    ] {
        super::claude_session_history::append_prompt(&mut app, &facts);
    }
    let rows: Vec<_> = app
        .session
        .messages()
        .iter()
        .map(|m| m.text.as_str())
        .collect();
    assert_eq!(
        rows,
        [
            "human",
            "[Source unknown] old",
            "Automatic prompt: automatic",
            "[Source unknown] "
        ]
    );
    assert_eq!(
        app.session.messages()[0].kind,
        carlos_core::session::models::MessageKind::UserSteer
    );
    assert_eq!(app.input_history_len(), 1);
    assert_eq!(app.session.claude_history().unknown().len(), 2);
}

#[test]
fn failed_start_after_switch_keeps_the_adopted_session_and_its_rows() {
    let mut app = super::state::AppState::new("original".into());
    app.session.begin_claude_edit("recover".into());
    super::claude_notifications::handle(
        &mut app,
        "claude/sessionChanged",
        json!({"generation":2,"sessionId":"branch"})
            .as_object()
            .expect("params"),
    );
    app.append_message(carlos_core::session::models::Role::Assistant, "branch row");
    let id = app.session.messages()[0].id;
    super::claude_notifications::handle(
        &mut app,
        "claude/sessionStartFailed",
        json!({"generation":2}).as_object().expect("params"),
    );
    assert_eq!(app.session.thread_id(), "branch");
    assert_eq!(app.session.messages()[0].id, id);
    assert_eq!(app.session.messages()[0].text, "branch row");
    assert_eq!(app.input_text(), "recover");
    assert_eq!(app.session.claude().phase(), ClaudePhase::Closed);
}

#[test]
fn notice_keys_keep_fifo_order_and_do_not_refresh_on_duplicates() {
    let mut app = super::state::AppState::new("session".into());
    for index in 0..128 {
        super::claude_notices::show(&mut app, format!("key-{index}"), format!("notice {index}"));
    }
    super::claude_notices::show(&mut app, "key-0".into(), "duplicate".into());
    super::claude_notices::show(&mut app, "key-128".into(), "notice 128".into());
    super::claude_notices::show(&mut app, "key-0".into(), "returned".into());
    assert_eq!(app.session.messages().len(), 130);
    assert_eq!(app.session.messages()[129].text, "returned");
    super::claude_reset::receive(&mut app, &json!({"new_conversation_id":"reset"}));
    super::claude_notices::show(&mut app, "key-0".into(), "still duplicate".into());
    assert!(app.session.messages().is_empty());
    super::claude_session_history::switch_session(&mut app, 2, "branch");
    super::claude_notices::show(&mut app, "key-0".into(), "new generation".into());
    assert_eq!(app.session.messages()[0].text, "new generation");
}

#[test]
fn native_warning_defaults_and_escaping_remain_exact() {
    let mut app = super::state::AppState::new("session".into());
    for facts in [
        json!({"subtype":"plugin_install","status":"failed"}),
        json!({"subtype":"hook_response"}),
        json!({"subtype":"informational","content":""}),
        json!({"subtype":"init","plugin_errors":[{}]}),
        json!({"subtype":"informational","noticeKey":"warning","content":"line\n\u{001b}"}),
        json!({"subtype":"informational","noticeKey":"warning","content":"duplicate"}),
        json!({"subtype":"plugin_install","status":"ok"}),
        json!({"subtype":"unknown"}),
    ] {
        super::claude_notices::receive(&mut app, &facts);
    }
    let rows: Vec<_> = app
        .session
        .messages()
        .iter()
        .map(|m| m.text.as_str())
        .collect();
    assert_eq!(
        rows,
        [
            "Plugin unknown failed: unknown error",
            "Hook unknown (unknown):  ",
            "Plugin unknown: could not load (unknown path)",
            "line\n\\u{1b}"
        ]
    );
}

#[test]
fn usage_notices_skip_invalid_rows_and_bound_input_before_filtering() {
    let mut app = super::state::AppState::new("session".into());
    let row = json!({"percent":25.0,"scope":{"model":{"display_name":"model\nname"},
        "surface":{"display_name":"ignored"}}});
    let mut rows = vec![
        json!({"percent":-1}),
        json!({"percent":101}),
        json!({"percent":"25"}),
        row,
    ];
    rows.extend((4..129).map(|_| json!({"percent":100})));
    super::claude_notices::usage(
        &mut app,
        &json!({"uuid":"usage","usage_report":{"rate_limits":{"limits":rows}}}),
    );
    let lines: Vec<_> = app.session.messages()[0].text.lines().collect();
    assert_eq!(lines.len(), 125);
    assert_eq!(
        lines[0],
        "quota model\\nname: 25.0% used; resets unknown (unknown)"
    );
    assert_eq!(lines[124], "quota : 100.0% used; resets unknown (unknown)");
}
