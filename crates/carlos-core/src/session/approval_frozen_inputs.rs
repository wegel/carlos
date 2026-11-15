//! Recreate the old native request inputs without consulting their frozen reply bodies.

use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::{Approval, ApprovalInput, Decision};
use serde_json::{json, Value};

use super::state::SessionState;

pub(super) const METHODS: [&str; 7] = [
    "item/commandExecution/requestApproval",
    "item/fileChange/requestApproval",
    "item/permissions/requestApproval",
    "execCommandApproval",
    "applyPatchApproval",
    "item/tool/requestUserInput",
    "mcpServer/elicitation/request",
];

pub(super) fn family(case: &str) -> String {
    let parts: Vec<_> = case.split('/').collect();
    match parts[0] {
        "claude" | "replace" => format!("claude/{}", parts[1]),
        "codex-guidance" => case.to_owned(),
        _ => case
            .rsplit_once('/')
            .expect("native lifetime tail")
            .0
            .to_owned(),
    }
}

pub(super) fn replies(family: &str, id: &str) -> Vec<Value> {
    let mut replies = Vec::new();
    for changed in [false, true] {
        let core = request(family, id, changed);
        replies.extend(convert_replies(&core));
    }
    replies
}

pub(super) fn view_replies(case: &Value) -> Vec<Value> {
    let mut core = SessionState::new("thread".into(), Default::default(), false);
    if let Some(method) = case["codexMethod"].as_str() {
        core.enable_codex();
        let mut params = server_params();
        params["command"] = json!("pwd");
        params["questions"] = json!([{"id":"q","header":"Header","question":"Question?",
            "options":[{"label":"Yes"},{"label":"No"}]}]);
        core.accept_codex_callback(method, &json!({"id":"request","params":params}));
    } else {
        let payload = if case["nativeFields"] == true {
            json!({"toolName":"AskUserQuestion","defaultToNo":true,"input":{"questions":[{
                "question":"Format?","header":"Format","options":[{"label":"Summary"}]}]}})
        } else {
            json!({"toolName":"Bash","defaultToNo":true})
        };
        core.receive_claude_callback(
            &json!({"callbackId":"request","kind":"permission","payload":payload}),
        );
    }
    convert_replies(&core)
        .into_iter()
        .map(|call| json!([call[1], call[2]]))
        .collect()
}

fn convert_replies(core: &SessionState) -> Vec<Value> {
    let card = core.export_approvals().remove(0);
    let facts = core
        .approval()
        .pending()
        .expect("original callback")
        .facts();
    answers(&card)
        .into_iter()
        .filter_map(|answer| {
            super::control_answers::convert(facts, &card, &answer)
                .ok()
                .map(|(_, body)| json!(["answer", facts.request_id, body]))
        })
        .collect()
}

fn request(family: &str, id: &str, changed: bool) -> SessionState {
    let mut core = SessionState::new("thread".into(), Default::default(), false);
    if let Some(index) = family.strip_prefix("claude/") {
        let (kind, mut payload) = claude_payload(index.parse().expect("Claude kind"));
        if changed {
            payload["input"]["command"] = json!("echo different request");
        }
        core.receive_claude_callback(&json!({"callbackId":id,"kind":kind,"payload":payload}));
    } else {
        core.set_codex_enabled_for_test(family.starts_with("Codex/"));
        let (method, params) = if let Some(index) = family.strip_prefix("codex-guidance/") {
            guidance(index.parse().expect("guidance kind"))
        } else {
            (
                family.split_once('/').expect("native method").1,
                server_params(),
            )
        };
        core.accept_codex_callback(method, &json!({"id":id,"params":params}));
    }
    core
}

fn answers(card: &Approval) -> Vec<ApprovalAnswer> {
    let mut answers: Vec<_> = [
        Decision::Once,
        Decision::Session,
        Decision::Deny,
        Decision::Cancel,
    ]
    .into_iter()
    .map(|decision| ApprovalAnswer::Decision { decision })
    .collect();
    match card.input.as_ref() {
        Some(ApprovalInput::Form { .. }) => {
            for value in [json!({}), json!({"count":2}), json!({"count":22})] {
                answers.push(ApprovalAnswer::Input { value });
            }
        }
        Some(ApprovalInput::Fields { fields }) => {
            for mode in 0..3 {
                let fields: Vec<_> = fields.iter().map(|field| {
                    let values = if mode == 0 {
                        field.options.first().map(|option| vec![option.value.clone()]).unwrap_or_default()
                    } else { Vec::new() };
                    json!({"key":field.key,"values":values,"text":if mode == 1 {Some("2")} else {None}})
                }).collect();
                answers.push(ApprovalAnswer::Input {
                    value: json!({"fields":fields}),
                });
            }
        }
        _ => {}
    }
    answers
}

fn claude_payload(index: usize) -> (&'static str, Value) {
    match index {
        0 => (
            "permission",
            json!({"toolName":"Bash","input":{"command":"echo old"}}),
        ),
        1 => (
            "permission",
            json!({"toolName":"ExitPlanMode","input":{"plan":"Plan","planFilePath":"plan.txt"}}),
        ),
        2 => (
            "permission",
            json!({"toolName":"AskUserQuestion","input":{"questions":[{
            "question":"Color?","header":"Color","options":[{"label":"Blue"}]}]}}),
        ),
        3 => (
            "elicitation",
            json!({"serverName":"server","requestedSchema":{"type":"object","properties":{}}}),
        ),
        4 => (
            "elicitation",
            json!({"serverName":"server","requestedSchema":{"type":"object",
            "properties":{"count":{"type":"integer"}},"required":["count"]}}),
        ),
        5 => (
            "elicitation",
            json!({"serverName":"server","mode":"url","url":"https://example.com/path","urlHost":"example.com"}),
        ),
        6 => (
            "elicitation",
            json!({"serverName":"server","requestedSchema":{"type":"object",
            "properties":{"nested":{"type":"object"}}}}),
        ),
        _ => panic!("unknown native payload {index}"),
    }
}

fn server_params() -> Value {
    json!({"threadId":"thread","turnId":"turn","itemId":"item","conversationId":"thread",
        "callId":"item","permissions":{},"command":"echo different","reason":"reason","plan":"Plan",
        "serverName":"server","mode":"form","requestedSchema":{"type":"object","properties":{}},
        "questions":[{"id":"q","header":"Header","question":"Color?","options":[{"label":"Blue"}]}]})
}

fn guidance(index: usize) -> (&'static str, Value) {
    match index {
        0 | 1 => (
            METHODS[5],
            json!({"threadId":"thread","turnId":"turn","itemId":"item",
            "isBlocking":index != 0,"questions":[{"id":"q","header":"Header","question":"Color?",
            "options":if index == 0 {json!([{"label":"Blue"}])} else {json!([])}}]}),
        ),
        2 | 3 => (
            METHODS[6],
            json!({"threadId":"thread","serverName":"server","mode":"url",
            "url":if index == 2 {"https://example.com/path"} else {"file:///bad"},"message":"Browser request"}),
        ),
        4 => (
            METHODS[6],
            json!({"threadId":"thread","serverName":"server","mode":"form",
            "requestedSchema":{"type":"object","properties":{"nested":{"type":"object"}}}}),
        ),
        5 => (
            METHODS[6],
            json!({"threadId":"thread","serverName":"server","mode":"unknown"}),
        ),
        _ => panic!("unknown guidance {index}"),
    }
}
