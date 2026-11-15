//! Preserve native reply clauses from unchanged pre-split fixtures, independent of old overlay hashes.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use super::approval_frozen_inputs::{family, replies, view_replies};
use super::approval_state::{ApprovalChoice, ApprovalRequestDraft, ApprovalRequestKind};

#[test]
fn all_frozen_lifetime_native_answers_keep_their_original_envelopes() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/m4_approval_lifetimes.json"))
            .expect("unchanged 2cbfedc native oracle");
    assert_eq!(cases.len(), 470);
    let mut generated = BTreeMap::new();
    let mut native = BTreeSet::new();
    let mut controls = BTreeSet::new();
    let mut answered = 0;
    for case in cases {
        let source = family(case["case"].as_str().expect("case label"));
        for step in case["steps"].as_array().expect("lifetime steps") {
            for call in step["calls"].as_array().expect("recorded writes") {
                if call[0] != "answer" {
                    controls.insert(call.to_string());
                    continue;
                }
                let id = call[1].as_str().expect("original native id");
                let actual = generated
                    .entry((source.clone(), id.to_owned()))
                    .or_insert_with(|| replies(&source, id));
                assert!(
                    actual.contains(call),
                    "{}: missing exact native reply {call}",
                    case["case"]
                );
                native.insert(call.to_string());
                answered += 1;
            }
        }
    }
    assert_eq!(
        answered, 1875,
        "retain every cumulative native-answer clause"
    );
    assert_eq!(
        native.len(),
        44,
        "retain every distinct exact id/body tuple"
    );
    assert_eq!(controls, expected_controls());
}

// These three ordinary controls have dedicated guarded engine/actual PTY successors;
// the native reply test above never pretends a callback reply is an ordinary send.
fn expected_controls() -> BTreeSet<String> {
    [
        json!(["reconnect", "thread"]),
        json!(["turn/interrupt", {"threadId":"thread","turnId":"turn"}]),
        json!(["turn/start", {"input":[{"text":"/clear","text_elements":[],"type":"text"}],"threadId":"thread"}]),
    ].into_iter().map(|call| call.to_string()).collect()
}

#[test]
fn all_frozen_view_native_answers_keep_their_original_envelopes() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("fixtures/m4_view_expected.json"))
        .expect("unchanged pre-split view oracle");
    assert_eq!(cases.len(), 273);
    let mut native = BTreeSet::new();
    let mut answered = 0;
    for case in cases {
        let generated = if let Some(kind) = case["kind"].as_str() {
            synthetic_replies(kind)
        } else {
            view_replies(&case)
        };
        for step in case["steps"].as_array().expect("view steps") {
            for call in step["calls"].as_array().expect("native writes") {
                assert!(
                    generated.contains(call),
                    "missing original view reply {call}"
                );
                assert_eq!(call[0], "request", "preserve the original native target");
                native.insert(call.to_string());
                answered += 1;
            }
        }
    }
    assert_eq!(native.len(), 12, "retain all distinct native view replies");
    assert_eq!(answered, 204, "retain every cumulative native view reply");
}

fn synthetic_replies(name: &str) -> Vec<Value> {
    let mut replies = Vec::new();
    for kind in [
        ApprovalRequestKind::CommandExecution,
        ApprovalRequestKind::FileChange,
        ApprovalRequestKind::Permissions,
        ApprovalRequestKind::LegacyExecCommand,
        ApprovalRequestKind::LegacyApplyPatch,
        ApprovalRequestKind::ClaudeExitPlanMode,
        ApprovalRequestKind::ClaudeNative,
        ApprovalRequestKind::ClaudeQuestion,
        ApprovalRequestKind::CodexQuestion,
        ApprovalRequestKind::CodexForm,
    ] {
        if format!("{kind:?}") != name {
            continue;
        }
        let request = synthetic_request(kind);
        for choice in [ApprovalChoice::Accept, ApprovalChoice::Decline] {
            if let Some(body) = request.response_for_choice(choice) {
                replies.push(json!([request.request_id, body]));
            }
        }
        // Native form cancellation is a separate original view clause.
        if let Some(body) = request.response_for_choice(ApprovalChoice::Cancel) {
            replies.push(json!([request.request_id, body]));
        }
    }
    replies
}

fn synthetic_request(kind: ApprovalRequestKind) -> ApprovalRequestDraft {
    // Exact original view fixture input, including permissions that look like UI metadata.
    ApprovalRequestDraft {
        request_id: json!("request"),
        method: "claude/native".into(),
        kind,
        title: format!("Approve {kind:?}"),
        detail_lines: vec!["native request details".into()],
        requested_permissions: Some(json!({"defaultToNo":true,"toolName":"Bash",
            "requestedSchema":{"type":"object","properties":{}}})),
        can_accept_for_session: true,
        can_decline: true,
        can_cancel: true,
    }
}
