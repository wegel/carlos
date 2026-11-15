//! Keep every child envelope private while preserving main human anchors, tools and reports.

use serde_json::{json, Value};

use super::backend_test_support::normalize;
use super::claude::ClaudePhase;
use super::models::Role;
use super::state::SessionState;
use crate::claude_backend::{is_child_report_notice, FixtureNormalizer};

fn frame(
    core: &mut SessionState,
    decoder: &mut FixtureNormalizer,
    frame: Value,
    method: Option<&str>,
) {
    let _ = normalize(
        core,
        decoder,
        &json!({"method":"claude/controller","params":{"frame":frame,"requestMethod":method}}),
    );
}

fn sdk(core: &mut SessionState, decoder: &mut FixtureNormalizer, message: Value) {
    frame(
        core,
        decoder,
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
    let mut core = SessionState::new("session".into(), Default::default(), false);
    let mut decoder = FixtureNormalizer::default();
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
            sdk(&mut core, &mut decoder, message);
        }
    }
    assert!(core.messages().is_empty());
    assert_eq!(core.thread_id(), "session");
    assert!(core.runtime_settings().current_model().is_none());
    assert!(core.claude().activity().thinking().is_none());
    assert!(core.active_turn_id().is_none());
    assert!(core.claude().phase() == ClaudePhase::Ready);
}

#[test]
fn main_tools_reports_and_child_callbacks_survive_without_task_detail_rows() {
    let mut core = SessionState::new("session".into(), Default::default(), false);
    let mut decoder = FixtureNormalizer::default();
    sdk(
        &mut core,
        &mut decoder,
        json!({"type":"assistant","message":{"id":"main-tool","role":"assistant","content":[
        {"type":"tool_use","id":"main-call","name":"Bash","input":{"command":"echo MAIN_COMMAND","agent_id":"tool-argument"}}]}}),
    );
    sdk(
        &mut core,
        &mut decoder,
        json!({"type":"user","message":{"role":"user","content":[
        {"type":"tool_result","tool_use_id":"main-call","content":"MAIN_OUTPUT"}]}}),
    );
    sdk(&mut core, &mut decoder, assistant("MAIN_REPORT"));
    sdk(
        &mut core,
        &mut decoder,
        json!({"type":"system","subtype":"task_progress","agent_id":"child",
        "task_id":"child","run_id":"z","description":"PANEL_DESCRIPTION","summary":"PANEL_SUMMARY"}),
    );
    frame(
        &mut core,
        &mut decoder,
        json!({"type":"callback","callbackId":"child-permission","kind":"permission",
        "payload":{"toolName":"Bash","input":{"command":"echo child"},"options":{"agentId":"child"}}}),
        None,
    );
    let text = core
        .messages()
        .iter()
        .map(|message| message.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for visible in ["MAIN_COMMAND", "MAIN_OUTPUT", "MAIN_REPORT"] {
        assert!(text.contains(visible), "{text}");
    }
    assert!(!text.contains("PANEL_"));
    assert_eq!(
        core.claude().tasks().data("child").expect("task")["summary"],
        "PANEL_SUMMARY"
    );
    assert_eq!(
        core.approval()
            .pending()
            .expect("child callback")
            .facts()
            .request_id,
        "child-permission"
    );
}

#[test]
fn history_retains_raw_child_metadata_without_transcript_or_rewind_entries() {
    let mut core = SessionState::new("session".into(), Default::default(), false);
    let mut decoder = FixtureNormalizer::default();
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
        &mut core,
        &mut decoder,
        json!({"type":"reply","ok":true,"result":{"messages":rows,"nextOffset":null}}),
        Some("sessions.read"),
    );
    assert_eq!(core.claude_history().records().len(), 10);
    assert!(core.claude_history().unknown().is_empty());
    assert_eq!(core.messages().len(), 2);
    assert_eq!(core.messages()[0].role, Role::User);
    assert_eq!(core.messages()[1].text, "MAIN_HISTORY_REPORT");
    assert_eq!(core.input_history().history_len(), 1);
    assert_eq!(
        core.input_history().text(0),
        "Human <task-notification> literal"
    );
    assert_eq!(
        core.input_history().entry_id(0),
        Some(core.messages()[0].id)
    );
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
        assert_eq!(is_child_report_notice(&notice), expected);
        let mut report = assistant("Main agent report");
        report["origin"] = notice["origin"].clone();
        assert!(!is_child_report_notice(&report));
        let mut core = SessionState::new("session".into(), Default::default(), false);
        let mut decoder = FixtureNormalizer::default();
        let mut record = notice;
        record["session_id"] = json!("session");
        record["uuid"] = json!("notice");
        frame(
            &mut core,
            &mut decoder,
            json!({"type":"reply","ok":true,"result":{"messages":[{"source":"automatic","message":record}],"nextOffset":null}}),
            Some("sessions.read"),
        );
        assert_eq!(core.messages().is_empty(), expected);
        assert_eq!(core.claude_history().records().len(), 1);
    }
}
