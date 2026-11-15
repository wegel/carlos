//! Keep native completion authoritative when the send worker reports its write afterward.

use std::time::Duration;

use serde_json::{json, Value};

use super::receiver_test_support::client;
use super::translate::finish_prompt_turn;
use super::GrokTranslationState;
use crate::backend::{BackendClient, BackendKind, PromptOrigin};
use crate::session::prompt_commands::{PromptContext, PromptOptions};
use crate::session::state::SessionState;

#[test]
fn empty_grok_result_before_send_settlement_cannot_restart_the_turn() {
    check_late_acknowledgement(Vec::new());
}

#[test]
fn grok_text_result_before_send_settlement_cannot_restart_the_turn() {
    check_late_acknowledgement(vec![json!({"method":"session/update","params":{
        "sessionId":"root","update":{"sessionUpdate":"agent_message_chunk",
            "content":{"type":"text","text":"instant answer"}}}})]);
}

#[test]
fn explicit_empty_grok_completion_before_send_settlement_cannot_restart_the_turn() {
    check_late_acknowledgement(vec![json!({"method":"_x.ai/session/update","params":{
        "sessionId":"root","update":{"sessionUpdate":"turn_completed"}}})]);
}

#[test]
fn unsolicited_grok_completion_before_any_prompt_does_not_start_a_turn() {
    let output = finish_prompt_turn(&mut GrokTranslationState::default(), false);
    assert!(output.lines.iter().all(|line| {
        serde_json::from_str::<Value>(line).expect("native fact")["method"] != "turn/started"
    }));
}

fn check_late_acknowledgement(frames: Vec<Value>) {
    let (client, events) = client(frames);
    let commands = client.capture_commands().expect("original Grok connection");
    let mut state = SessionState::new("root".into(), Default::default(), false);
    for text in ["first prompt", "second prompt"] {
        let work = state
            .prepare_turn_text(
                BackendKind::Grok,
                text.into(),
                PromptOptions {
                    record_history: true,
                    force_start: false,
                    origin: PromptOrigin::Human,
                },
                &PromptContext {
                    operation: None,
                    claude_queue: false,
                },
            )
            .expect("prepare original prompt")
            .work
            .expect("prompt dispatch");
        let completion = work.execute(&commands);
        loop {
            let line = events
                .recv_timeout(Duration::from_secs(5))
                .expect("native completion");
            state.handle_backend_line(&line, &state.backend_context());
            if serde_json::from_str::<Value>(&line).expect("native fact")["method"]
                == "turn/completed"
            {
                break;
            }
        }
        assert!(
            state.active_turn_id().is_none(),
            "native result finishes the turn"
        );
        state
            .finish_backend_work(completion)
            .expect("settle the captured write");
        assert!(
            state.active_turn_id().is_none(),
            "late write acknowledgement cannot restart {text}"
        );
    }
}
