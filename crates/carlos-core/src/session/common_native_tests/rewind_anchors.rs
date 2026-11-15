//! Keep native fork anchors exact across backends, live echoes and loaded history.

use carlos_proto::submissions::ClientOperation;
use serde_json::{json, Value};

use super::{feed, session};
use crate::backend::{BackendKind, RewindForkRequest, RewindUserMessage};
use crate::session::claude::ClaudePhase;
use crate::session::input_contract_test_support::{peer, Writes};
use crate::session::item_history::load_history_from_start_or_resume;
use crate::session::models::MessageKind;
use crate::session::prompt_commands::PromptContext;
use crate::session::state::SessionState;

#[test]
fn each_backend_receives_the_original_anchor_before_the_edited_prompt() {
    for kind in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        let (commands, writes) = peer(kind);
        let mut state = seed(kind);
        let selected = state.input_history().prompt_id(1).expect("second prompt");
        let mut prepared = state
            .prepare_rewind_text(
                kind,
                selected,
                "edited".into(),
                &PromptContext {
                    operation: Some(ClientOperation {
                        client: "a".repeat(32).parse().expect("client"),
                        id: "1".parse().expect("operation"),
                    }),
                    claude_queue: false,
                },
                false,
            )
            .expect("prepare fork");
        while let Some(work) = prepared.work {
            prepared = state
                .finish_backend_work(work.execute(&commands))
                .expect("original completion");
        }
        let writes = writes.lock().expect("native writes");
        assert_fork(kind, &writes);
        if kind == BackendKind::Claude {
            assert!(writes.starts.is_empty());
            assert!(state.claude_edit_progress().is_some());
        } else {
            assert_submission(kind, &writes);
            assert_eq!(state.input_history().history_len(), 2);
            assert_eq!(state.input_history().text(0), "first");
            assert_eq!(state.input_history().text(1), "edited");
        }
    }
}

#[test]
fn invisible_native_users_keep_exact_positions_in_live_and_replaced_history() {
    for loaded in [false, true] {
        for hidden in [0, 1, 3] {
            let mut state = session("root");
            state.enable_native_history();
            load_users(&mut state, hidden, loaded);
            assert_eq!(state.input_history().history_len(), 1);
            let entry = state.input_history().entry_id(0).expect("visible prompt");
            assert_eq!(state.input_history().turn_id(entry), Some("shared"));
            assert_eq!(
                state.input_history().native_request(0, entry),
                Some(RewindUserMessage {
                    item_id: "visible".into(),
                    text: "visible".into(),
                    position: hidden,
                })
            );
            let selected = state.messages().position(entry).expect("visible row");
            assert_eq!(
                state.messages()[selected].kind,
                if hidden == 0 {
                    MessageKind::Plain
                } else {
                    MessageKind::UserSteer
                }
            );
        }
    }
}

fn seed(kind: BackendKind) -> SessionState {
    let mut state = session("root");
    state.set_claude_phase_for_test(ClaudePhase::Ready);
    if kind == BackendKind::Codex {
        state.enable_native_history();
    }
    for (turn, id, text) in [("t1", "u1", "first"), ("t2", "u2", "second")] {
        let item = json!({"type":"userMessage","id":id,
            "content":[{"type":"text","text":text}]});
        receive(&mut state, turn, item, &["item/started", "item/completed"]);
    }
    state
}

fn assert_fork(kind: BackendKind, writes: &Writes) {
    let native = (kind == BackendKind::Codex).then(|| RewindUserMessage {
        item_id: "u2".into(),
        text: "second".into(),
        position: 0,
    });
    assert_eq!(
        writes.forks,
        vec![(
            "root".into(),
            RewindForkRequest {
                keep_turns: 1,
                drop_turns: 1,
                before_turn_id: Some("t2".into()),
                before_user_item: native,
            }
        )],
        "{kind:?}"
    );
}

fn assert_submission(kind: BackendKind, writes: &Writes) {
    let mut starts = writes.starts.clone();
    if kind == BackendKind::Codex {
        let token = starts[0]
            .as_object_mut()
            .expect("params")
            .remove("clientUserMessageId")
            .expect("exact native echo token");
        uuid::Uuid::parse_str(token.as_str().expect("UUID string")).expect("valid echo token");
    }
    let root = if kind == BackendKind::Grok {
        "grok-fork"
    } else {
        "thread-fork"
    };
    assert_eq!(
        starts,
        vec![json!({"threadId":root,
        "input":[{"type":"text","text":"edited","text_elements":[]}]})]
    );
}

fn load_users(state: &mut SessionState, hidden: usize, loaded: bool) {
    let mut items: Vec<_> = (0..hidden)
        .map(|index| json!({"type":"userMessage","id":format!("hidden-{index}"),"content":[]}))
        .collect();
    items.push(json!({"type":"userMessage","id":"visible",
        "content":[{"type":"text","text":"visible"}]}));
    if loaded {
        let response = json!({"result":{"thread":{"id":"root","turns":[
            {"id":"shared","status":"completed","items":items}]}}});
        load_history_from_start_or_resume(state, &response.to_string()).expect("history");
    } else {
        for item in items {
            receive(
                state,
                "shared",
                item,
                &["item/started", "item/completed", "item/completed"],
            );
        }
    }
}

fn receive(state: &mut SessionState, turn: &str, item: Value, methods: &[&str]) {
    for method in methods {
        feed(
            state,
            &json!({"method":method,"params":{
            "threadId":"root","turnId":turn,"item":item}})
            .to_string(),
        );
    }
}
