//! Keeps private Claude workstreams out of live and loaded main transcripts.

use serde_json::{json, Value};

use super::claude_state::ClaudePhase;
use super::models::Role;
use super::notifications::handle_server_message_line;
use super::state::AppState;

fn frame(app: &mut AppState, frame: Value, method: Option<&str>) {
    let _ = handle_server_message_line(
        app,
        &json!({"method":"claude/controller",
        "params":{"frame":frame,"requestMethod":method}})
        .to_string(),
    );
}

fn sdk(app: &mut AppState, message: Value) {
    frame(
        app,
        json!({"type":"event","event":"sdk","message":message}),
        None,
    );
}

fn assistant(text: &str) -> Value {
    json!({"type":"assistant","session_id":"session","uuid":text,
        "message":{"id":text,"role":"assistant","content":[{"type":"text","text":text}]}})
}

fn child_tags() -> Vec<Value> {
    vec![
        json!({"agent_id":"child"}),
        json!({"parent_tool_use_id":"caller"}),
        json!({"parent_agent_id":"nested-parent"}),
        json!({"isSidechain":true}),
    ]
}

#[test]
fn every_child_envelope_hides_text_thinking_tools_results_and_session_state() {
    let mut app = AppState::new("session".to_string());
    for tags in child_tags() {
        for mut message in [
            assistant("PRIVATE_CHILD"),
            json!({"type":"assistant","message":{"id":"child-tool","role":"assistant","content":[
                {"type":"thinking","thinking":"PRIVATE_THOUGHT"},
                {"type":"tool_use","id":"child-call","name":"Bash","input":{"command":"PRIVATE_COMMAND"}}]}}),
            json!({"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"child-call","content":"PRIVATE_RESULT"}]}}),
            json!({"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":"PRIVATE_STREAM"}}}),
            json!({"type":"system","subtype":"thinking_tokens","estimated_tokens":999}),
            json!({"type":"system","subtype":"session_state_changed","state":"running"}),
            json!({"type":"conversation_reset","session_id":"session","new_conversation_id":"private-reset"}),
            json!({"type":"system","subtype":"init","session_id":"child-session","model":"private-model"}),
            json!({"type":"result","subtype":"error","is_error":true,"result":"PRIVATE_FAILURE"}),
        ] {
            message
                .as_object_mut()
                .expect("message")
                .extend(tags.as_object().expect("tags").clone());
            sdk(&mut app, message);
        }
    }
    assert!(app.messages.is_empty());
    assert_eq!(app.thread_id, "session");
    assert!(app.runtime.current_model.is_none());
    assert!(app.claude.activity.thinking.is_none());
    assert!(app.active_turn_id.is_none());
    assert!(app.claude.phase == ClaudePhase::Ready);
}

#[test]
fn main_tools_reports_and_child_callbacks_survive_without_task_detail_rows() {
    let mut app = AppState::new("session".to_string());
    sdk(
        &mut app,
        json!({"type":"assistant","message":{"id":"main-tool","role":"assistant","content":[
        {"type":"tool_use","id":"main-call","name":"Bash","input":{"command":"echo MAIN_COMMAND","agent_id":"tool-argument"}}]}}),
    );
    sdk(
        &mut app,
        json!({"type":"user","message":{"role":"user","content":[
        {"type":"tool_result","tool_use_id":"main-call","content":"MAIN_OUTPUT"}]}}),
    );
    sdk(&mut app, assistant("MAIN_REPORT"));
    sdk(
        &mut app,
        json!({"type":"system","subtype":"task_progress","agent_id":"child",
        "task_id":"child","run_id":"z","description":"PANEL_DESCRIPTION","summary":"PANEL_SUMMARY"}),
    );
    frame(
        &mut app,
        json!({"type":"callback","callbackId":"child-permission","kind":"permission",
        "payload":{"toolName":"Bash","input":{"command":"echo child"},"options":{"agentId":"child"}}}),
        None,
    );
    let text = app
        .messages
        .iter()
        .map(|message| message.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for visible in ["MAIN_COMMAND", "MAIN_OUTPUT", "MAIN_REPORT"] {
        assert!(text.contains(visible), "{text}");
    }
    assert!(!text.contains("PANEL_"));
    assert_eq!(
        app.claude.tasks.data("child").expect("task")["summary"],
        "PANEL_SUMMARY"
    );
    assert_eq!(
        app.approval
            .pending
            .as_ref()
            .expect("child callback")
            .request_id,
        "child-permission"
    );
}

#[test]
fn history_retains_raw_child_metadata_without_transcript_or_rewind_entries() {
    let mut app = AppState::new("session".to_string());
    let mut rows = Vec::new();
    for tags in child_tags() {
        for mut message in [
            assistant("PRIVATE_HISTORY"),
            json!({"type":"user","session_id":"session",
            "uuid":"child-user","message":{"content":"PRIVATE_PROMPT"}}),
        ] {
            message
                .as_object_mut()
                .expect("message")
                .extend(tags.as_object().expect("tags").clone());
            rows.push(json!({"source":"human","message":message}));
        }
    }
    rows.push(
        json!({"source":"human","message":{"type":"user","session_id":"session","uuid":"human",
        "origin":{"kind":"human"},"message":{"content":"Human <task-notification> literal"}}}),
    );
    rows.push(json!({"source":"system","message":assistant("MAIN_HISTORY_REPORT")}));
    frame(
        &mut app,
        json!({"type":"reply","ok":true,"result":{"messages":rows,"nextOffset":null}}),
        Some("sessions.read"),
    );
    assert_eq!(app.claude.history.records.len(), 10);
    assert!(app.claude.history.unknown.is_empty());
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[0].role, Role::User);
    assert_eq!(app.messages[1].text, "MAIN_HISTORY_REPORT");
    app.enter_rewind_mode();
    assert_eq!(app.input_text(), "Human <task-notification> literal");
}

#[test]
fn only_positively_identified_child_report_notices_are_hidden() {
    let hidden = [
        json!({"kind":"task-notification","runId":"run"}),
        json!({"kind":"peer","senderTaskId":"child"}),
        json!({"kind":"observer","senderTaskId":"child"}),
    ];
    let visible = [
        json!({"kind":"human"}),
        json!({"kind":"auto-continuation"}),
        json!({"kind":"task-notification"}),
        json!({"kind":"task-notification","subkind":"scheduled-trigger","runId":"run"}),
        json!({"kind":"peer","fromSession":"other"}),
        json!({"kind":"channel"}),
        json!({"kind":"future"}),
    ];
    for (origin, expected) in hidden
        .into_iter()
        .map(|origin| (origin, true))
        .chain(visible.into_iter().map(|origin| (origin, false)))
    {
        let notice = json!({"type":"user","origin":origin,"message":{"content":"<task-notification>text</task-notification>"}});
        assert_eq!(
            crate::claude_backend::is_child_report_notice(&notice),
            expected
        );
        let mut report = assistant("Main agent report");
        report["origin"] = notice["origin"].clone();
        assert!(!crate::claude_backend::is_child_report_notice(&report));
        let mut app = AppState::new("session".to_string());
        let mut record = notice;
        record["session_id"] = json!("session");
        record["uuid"] = json!("notice");
        frame(
            &mut app,
            json!({"type":"reply","ok":true,"result":{"messages":[{"source":"automatic","message":record}],"nextOffset":null}}),
            Some("sessions.read"),
        );
        assert_eq!(app.messages.is_empty(), expected);
        assert_eq!(app.claude.history.records.len(), 1);
    }
}
