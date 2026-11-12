//! Compare consent cards with snapshots captured from the pre-split app.
// m4_view_expected.json is a frozen record of commit 2cbfedc behaviour, with
// non-rendered status fields/cases removed; it cannot be regenerated from this
// repository. The original capture depended on helpers under target/m4-fix/.

use crossterm::event::{KeyCode, KeyModifiers};
use serde_json::{json, Value};

use super::state::{AppState, ApprovalChoice, ApprovalRequestKind};
use super::view_hint_support::{key, request, snapshot, Client};
use crate::backend::BackendKind;

#[test]
fn approval_cards_match_heads_sequences() {
    let actual = sequences();
    let expected: Value = serde_json::from_str(include_str!("fixtures/m4_view_expected.json"))
        .expect("HEAD snapshots");
    assert_same_cases(&actual, &expected);
}

fn assert_same_cases(actual: &Value, expected: &Value) {
    let actual = actual.as_array().expect("cases");
    let expected = expected.as_array().expect("HEAD cases");
    assert_eq!(actual.len(), 273, "complete actual case corpus");
    assert_eq!(actual.len(), expected.len(), "same number of HEAD cases");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual, expected, "case {index}");
    }
}

pub(super) fn sequences() -> Value {
    let mut cases = Vec::new();
    for backend in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        for kind in kinds() {
            for tail in [
                "accept", "decline", "other", "focus", "arrive", "cancel", "resolve",
            ] {
                cases.push(approval_sequence(backend, kind, tail));
            }
        }
    }
    for fields in [false, true] {
        for tail in [
            "accept", "decline", "other", "focus", "arrive", "cancel", "resolve",
        ] {
            cases.push(native_sequence(fields, tail));
        }
    }
    for method in [
        "item/commandExecution/requestApproval",
        "item/fileChange/requestApproval",
        "item/permissions/requestApproval",
        "execCommandApproval",
        "applyPatchApproval",
        "item/tool/requestUserInput",
        "mcpServer/elicitation/request",
    ] {
        for tail in [
            "accept", "decline", "other", "focus", "arrive", "cancel", "resolve",
        ] {
            cases.push(codex_sequence(method, tail));
        }
    }
    json!(cases)
}

fn kinds() -> [ApprovalRequestKind; 10] {
    use super::state::ApprovalRequestKind as K;
    [
        K::CommandExecution,
        K::FileChange,
        K::Permissions,
        K::LegacyExecCommand,
        K::LegacyApplyPatch,
        K::ClaudeExitPlanMode,
        K::ClaudeNative,
        K::ClaudeQuestion,
        K::CodexQuestion,
        K::CodexForm,
    ]
}

fn approval_sequence(backend: BackendKind, kind: ApprovalRequestKind, tail: &str) -> Value {
    let client = Client::new(backend);
    let mut app = AppState::new("thread".into());
    app.session
        .set_claude_enabled_for_test(backend == BackendKind::Claude);
    app.set_pending_approval(request(kind, "request"));
    let mut steps = vec![snapshot(&client, &app)];
    super::turn_approval::respond_to_pending_approval(&client, &mut app, ApprovalChoice::Accept);
    steps.push(snapshot(&client, &app));
    approval_tail(&client, &mut app, tail);
    steps.push(snapshot(&client, &app));
    if matches!(tail, "other" | "focus" | "arrive" | "cancel" | "resolve") {
        super::turn_approval::respond_to_pending_approval(
            &client,
            &mut app,
            ApprovalChoice::Accept,
        );
        steps.push(snapshot(&client, &app));
    }
    json!({"backend":format!("{backend:?}"),"kind":format!("{kind:?}"),"tail":tail,"steps":steps})
}

fn approval_tail(client: &Client, app: &mut AppState, tail: &str) {
    match tail {
        "accept" => {
            super::turn_approval::respond_to_pending_approval(client, app, ApprovalChoice::Accept)
        }
        "decline" => {
            super::turn_approval::respond_to_pending_approval(client, app, ApprovalChoice::Decline)
        }
        "other" => key(client, app, KeyCode::Char('x'), KeyModifiers::NONE),
        "arrive" => super::claude_callbacks::receive_callback(
            app,
            &json!({"callbackId":"second",
            "kind":"permission","payload":{"toolName":"Bash","defaultToNo":true}}),
        ),
        "focus" => {
            super::claude_callbacks::receive_callback(
                app,
                &json!({"callbackId":"second",
                "kind":"permission","payload":{"toolName":"Bash","defaultToNo":true}}),
            );
            key(client, app, KeyCode::F(4), KeyModifiers::NONE);
            key(client, app, KeyCode::F(4), KeyModifiers::NONE);
        }
        "cancel" => super::claude_callbacks::cancel_callback(app, &json!("request")),
        "resolve" => app.clear_pending_approval(),
        _ => unreachable!(),
    }
}

fn native_sequence(fields: bool, tail: &str) -> Value {
    let client = Client::new(BackendKind::Claude);
    let mut app = AppState::new("thread".into());
    app.session.set_claude_enabled_for_test(true);
    let payload = if fields {
        json!({"toolName":"AskUserQuestion","defaultToNo":true,"input":{"questions":[{
            "question":"Format?","header":"Format","options":[{"label":"Summary"}]}]}})
    } else {
        json!({"toolName":"Bash","defaultToNo":true})
    };
    super::claude_callbacks::receive_callback(
        &mut app,
        &json!({"callbackId":"request","kind":"permission","payload":payload}),
    );
    let mut steps = vec![snapshot(&client, &app)];
    super::turn_approval::respond_to_pending_approval(&client, &mut app, ApprovalChoice::Accept);
    steps.push(snapshot(&client, &app));
    approval_tail(&client, &mut app, tail);
    steps.push(snapshot(&client, &app));
    if matches!(tail, "other" | "focus" | "arrive" | "cancel" | "resolve") {
        super::turn_approval::respond_to_pending_approval(
            &client,
            &mut app,
            ApprovalChoice::Accept,
        );
        steps.push(snapshot(&client, &app));
    }
    json!({"nativeFields":fields,"tail":tail,"steps":steps})
}

fn codex_sequence(method: &str, tail: &str) -> Value {
    let client = Client::new(BackendKind::Codex);
    let mut app = AppState::new("thread".into());
    app.codex.enabled = true;
    let params = json!({"threadId":"thread","turnId":"turn","itemId":"item",
        "conversationId":"thread","callId":"item","permissions":{},"command":"pwd",
        "serverName":"server","mode":"form","requestedSchema":{"type":"object","properties":{}},
        "questions":[{"id":"q","header":"Header","question":"Question?",
            "options":[{"label":"Yes"},{"label":"No"}]}]});
    super::codex_callbacks::receive(&mut app, method, &json!({"id":"request","params":params}));
    let mut steps = vec![snapshot(&client, &app)];
    super::turn_approval::respond_to_pending_approval(&client, &mut app, ApprovalChoice::Accept);
    steps.push(snapshot(&client, &app));
    if tail == "resolve" {
        super::codex_callbacks::resolved(
            &mut app,
            &json!({"threadId":"thread","requestId":"request"}),
        );
    } else if tail == "arrive" || tail == "focus" {
        super::codex_callbacks::receive(&mut app, method, &json!({"id":"second","params":params}));
        if tail == "focus" {
            key(&client, &mut app, KeyCode::F(4), KeyModifiers::NONE);
        }
    } else if tail == "cancel" {
        super::turn_approval::respond_to_pending_approval(
            &client,
            &mut app,
            ApprovalChoice::Cancel,
        );
    } else {
        approval_tail(&client, &mut app, tail);
    }
    steps.push(snapshot(&client, &app));
    json!({"codexMethod":method,"tail":tail,"steps":steps})
}

#[test]
fn parity_comparison_rejects_missing_trailing_or_empty_case_arrays() {
    let complete = json!(vec![Value::Null; 273]);
    for (actual, expected) in [
        (json!(vec![Value::Null; 272]), complete.clone()),
        (complete.clone(), json!(vec![Value::Null; 272])),
        (json!([]), json!([])),
        (complete.clone(), json!([])),
    ] {
        assert!(std::panic::catch_unwind(|| assert_same_cases(&actual, &expected)).is_err());
    }
}
