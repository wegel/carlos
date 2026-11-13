//! Read all three actual backend receivers and drive the session without terminal widgets.

use std::process::Command;
use std::time::Duration;

use serde_json::{json, Value};

use super::backend_test_support::receive;
use super::models::Role;
use super::state::SessionState;
use crate::backend::{BackendClient, PromptOrigin, PromptPriority};
use crate::protocol::AppServerClient;

#[test]
fn codex_receiver_drives_shared_facts_and_exact_error_reply_without_a_view() {
    let lines = codex_lines();
    let mut command = Command::new("python3");
    command.args([
        "-u",
        "-c",
        concat!(
            "import sys,json\n",
            "frames=json.loads(sys.argv[1]); last=None\n",
            "for line in sys.stdin:\n",
            " r=json.loads(line)\n",
            " if 'method' not in r: last=r; continue\n",
            " if r['method']=='fixture/probe':\n",
            "  for f in frames: print(json.dumps(f),flush=True)\n",
            " print(json.dumps({'id':r['id'],'result':last}),flush=True)\n"
        ),
        &json!(lines).to_string(),
    ]);
    let mut client = AppServerClient::start_command(command).expect("fixed Codex peer");
    let receiver = client.take_events_rx().expect("receiver");
    client
        .call("fixture/probe", json!({}), Duration::from_secs(2))
        .expect("all frames queued");
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    for line in receiver.try_iter() {
        receive(&mut session, &client, &line);
    }
    assert_answer(&session, "hello world");
    let reply = client
        .call("fixture/verify", json!({}), Duration::from_secs(2))
        .expect("callback response");
    let reply: Value = serde_json::from_str(&reply).expect("reply JSON");
    assert_eq!(reply["result"]["id"], "unknown-callback");
    assert_eq!(reply["result"]["error"]["code"], -32601);
    assert_eq!(session.context_usage().expect("native usage").used, 2500);
}

#[test]
fn claude_receiver_drives_named_and_common_facts_without_a_view() {
    let root = "00000000-0000-4000-8000-000000000001";
    let frames = vec![
        sdk(
            json!({"type":"system","subtype":"init","session_id":root,"user_message_uuid":"$prompt"}),
        ),
        sdk(
            json!({"type":"assistant","uuid":"message","message":{"id":"answer","role":"assistant","content":[{"type":"text","text":"hello world"}]}}),
        ),
        json!({"v":1,"type":"callback","callbackId":"permission","kind":"permission","payload":{"toolName":"Bash","defaultToNo":true}}),
        sdk(
            json!({"type":"result","subtype":"success","is_error":false,"user_message_uuid":"$prompt"}),
        ),
        sdk(json!({"type":"system","subtype":"session_state_changed","state":"idle"})),
    ];
    let (client, receiver) = crate::claude_backend::receiver_client(frames, json!({}), 1024);
    client
        .submit_prompt(
            "turn/start",
            json!({"input":[{"text":"hello"}]}),
            PromptOrigin::Human,
            PromptPriority::Queue,
            Duration::from_secs(2),
        )
        .expect("fixed prompt");
    let mut session = SessionState::new(root.into(), Default::default(), false);
    session.enable_claude();
    for line in receiver.try_iter() {
        assert!(
            !line.contains("claude/controller"),
            "only normalized records leave the backend"
        );
        receive(&mut session, &client, &line);
    }
    assert_answer(&session, "hello world");
    assert!(session
        .messages()
        .iter()
        .any(|row| row.role == Role::User && row.text == "hello"));
    assert_eq!(
        session
            .approval()
            .pending()
            .expect("native callback")
            .facts()
            .request_id,
        "permission"
    );
}

#[test]
fn grok_process_receiver_drives_common_facts_without_a_view() {
    let frames = vec![
        json!({"method":"session/update","params":{"sessionId":"root",
        "update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"hello world"}}}}),
    ];
    let (client, receiver) = crate::grok_backend::receiver_test_support::client(frames);
    let mut session = SessionState::new("root".into(), Default::default(), false);
    client
        .call(
            "turn/start",
            json!({"input":[{"text":"hello"}]}),
            Duration::from_secs(2),
        )
        .expect("fixed Grok prompt");
    loop {
        let line = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("native completion");
        let frame: Value = serde_json::from_str(&line).expect("common JSON");
        let complete = frame["method"] == "turn/completed";
        receive(&mut session, &client, &line);
        if complete {
            break;
        }
    }
    assert_answer(&session, "hello world");
}

fn assert_answer(session: &SessionState, text: &str) {
    let answer = session
        .messages()
        .iter()
        .find(|row| row.role == Role::Assistant)
        .expect("answer");
    assert_eq!(answer.text, text);
    assert_eq!(
        session.messages().position(answer.id),
        session
            .messages()
            .iter()
            .position(|row| row.id == answer.id)
    );
    assert!(
        session.active_turn_id().is_none(),
        "native completion settles the turn"
    );
}

fn sdk(message: Value) -> Value {
    json!({"v":1,"type":"event","event":"sdk","message":message})
}

fn codex_lines() -> Vec<Value> {
    vec![
        json!({"method":"turn/started","params":{"threadId":"root","turn":{"id":"turn","status":"inProgress"}}}),
        json!({"method":"item/completed","params":{"threadId":"root","turnId":"turn","item":{"id":"answer","type":"agentMessage","text":"hello world"}}}),
        json!({"method":"thread/tokenUsage/updated","params":{"threadId":"root","tokenUsage":{"modelContextWindow":10000,"total":{"totalTokens":2500}}}}),
        json!({"id":"unknown-callback","method":"future/question","params":{"threadId":"child"}}),
        json!({"method":"turn/completed","params":{"threadId":"root","turn":{"id":"turn","status":"completed"}}}),
    ]
}
