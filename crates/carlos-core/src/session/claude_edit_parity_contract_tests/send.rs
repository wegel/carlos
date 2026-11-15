//! Send exact human text and queue policy through the real Claude pipe and captured commands.

use std::time::Instant;

use serde_json::json;

use super::{advance, core, event, notices, recovered};
use crate::backend::{BackendClient, BackendKind, PromptOrigin, PromptPriority};
use crate::claude_backend::receiver_client;
use crate::session::command_work::{BackendCall, BackendWork, Finish, PreparedCommand};
use crate::session::prompt_commands::{PromptContext, PromptOptions};
use crate::session::state::SessionState;

/// Execute exactly the captured prompt work through a real controller pipe.
pub(in crate::session) fn submit(
    core: &mut SessionState,
    text: &str,
    options: PromptOptions,
    queue: bool,
    fail: bool,
) -> (PreparedCommand, Vec<(String, PromptOrigin, PromptPriority)>) {
    let result = json!({"_failedMethods":if fail {vec!["prompt"]} else {Vec::new()},"_failureMessage":"fixed send failure"});
    let (client, _events) = receiver_client(Vec::new(), result, 1024);
    core.set_claude_generation_for_test(
        client
            .connection_generation()
            .expect("actual helper generation"),
    );
    let mut prepared = core
        .prepare_turn_text(
            BackendKind::Claude,
            text.into(),
            options,
            &PromptContext {
                operation: None,
                claude_queue: queue,
            },
        )
        .expect("prepare original send");
    let Some(work) = prepared.work.take() else {
        return (prepared, Vec::new());
    };
    let trace = capture(&work, text);
    let completion = work.execute(
        &client
            .capture_commands()
            .expect("captured actual Claude commands"),
    );
    assert_eq!(completion.result.is_err(), fail);
    if let Ok(response) = &completion.result {
        uuid::Uuid::parse_str(
            response["result"]["turn"]["id"]
                .as_str()
                .expect("native UUID"),
        )
        .expect("real prompt UUID");
    }
    let finished = core
        .finish_backend_work(completion)
        .expect("settle original command");
    assert!(
        finished.work.is_none(),
        "native acknowledgement cannot resend"
    );
    (finished, trace)
}

/// Keep the caller history and forced-send choice on a human prompt.
pub(in crate::session) fn human(force_start: bool) -> PromptOptions {
    PromptOptions {
        record_history: true,
        force_start,
        origin: PromptOrigin::Human,
    }
}

#[test]
fn synchronous_send_success_and_failure_keep_the_composer_and_exact_notices() {
    for fail in [false, true] {
        let mut core = core();
        advance(&mut core, 2);
        let input = core
            .dequeue_turn_input(Instant::now())
            .expect("queued edit");
        let (finished, trace) = submit(&mut core, &input.text, human(true), false, fail);
        assert!(core.claude_edit_progress().is_none());
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].0, "edited text");
        if fail {
            assert_eq!(
                notices(&core),
                [
                    "Recovered unsent edited prompt:\nedited text",
                    "Could not edit Claude message: fixed send failure. Draft kept.",
                    "Could not send message: fixed send failure"
                ]
            );
            assert_eq!(recovered(&finished.update.changes).len(), 1);
            assert_eq!(recovered(&finished.update.changes)[0].0, "edited text");
        } else {
            assert_eq!(
                notices(&core),
                ["edited text", "Claude started the edited branch"]
            );
        }
    }
}

#[test]
fn human_steer_and_queue_choices_reach_backend_without_changing_automatic_source() {
    for queue in [false, true] {
        let mut core = core();
        core.record_claude_activity(&json!({"state":"running"}));
        let (_, trace) = submit(&mut core, "human text", human(false), queue, false);
        assert_eq!(trace.len(), 1);
        assert!(trace[0].1 == PromptOrigin::Human);
        assert!(
            trace[0].2
                == if queue {
                    PromptPriority::Queue
                } else {
                    PromptPriority::Steer
                }
        );
    }
}

#[test]
fn slash_clear_obeys_startup_and_queue_guards_at_every_edit_stage() {
    for stage in 0..5 {
        let mut core = core();
        advance(&mut core, stage);
        let (_, trace) = submit(&mut core, "/clear", human(false), false, false);
        assert!(
            core.claude_edit_progress().is_some(),
            "clear alone cannot settle edit"
        );
        assert_eq!(trace.len(), usize::from(stage >= 3));
        let uuid = if stage >= 3 {
            let id = core
                .active_turn_id()
                .as_deref()
                .expect("actual clear UUID")
                .to_owned();
            uuid::Uuid::parse_str(&id).expect("real native prompt UUID");
            assert_eq!(notices(&core), ["/clear"]);
            id
        } else {
            "clear-uuid".into()
        };
        event(
            &mut core,
            "claude/conversationReset",
            json!({"new_conversation_id":"reset","user_message_uuid":uuid}),
        );
        assert!(core.claude_edit_progress().is_none());
        assert!(core.messages().is_empty());
        let mut queued = Vec::new();
        while let Some(input) = core.dequeue_turn_input(Instant::now()) {
            queued.push(input.text);
        }
        assert_eq!(
            queued,
            match stage {
                0 | 1 => vec!["/clear"],
                2 => vec!["edited text", "/clear"],
                _ => Vec::new(),
            }
        );
    }
}

fn capture(work: &BackendWork, text: &str) -> Vec<(String, PromptOrigin, PromptPriority)> {
    match (&work.call, &work.finish) {
        (
            BackendCall::Prompt {
                params, priority, ..
            },
            Finish::Prompt(submission),
        ) => {
            assert_eq!(params["input"][0]["text"], text);
            vec![(text.to_string(), submission.options.origin, *priority)]
        }
        _ => panic!("one native prompt work item"),
    }
}
