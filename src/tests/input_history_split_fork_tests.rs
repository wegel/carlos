//! Check exact backend rewind targets and navigation after the backend accepts a fork.

use std::sync::Mutex;
use std::time::Duration;

use anyhow::{bail, Result};
use carlos_core::backend::{
    BackendClient, BackendKind, EventReceiver, RewindForkRequest, RewindUserMessage,
};
use carlos_core::session::models::MessageKind;
use serde_json::{json, Value};

use crate::app::item_history::load_history_from_start_or_resume;
use crate::app::notifications::handle_notification_line;
use crate::app::state::AppState;
use crate::app::turn_submit::submit_rewind_turn_text;

struct Peer {
    kind: BackendKind,
    calls: Mutex<Vec<Value>>,
    forks: Mutex<Vec<(String, RewindForkRequest)>>,
}

impl BackendClient for Peer {
    fn kind(&self) -> BackendKind {
        self.kind
    }

    fn call(&self, method: &str, params: Value, _: Duration) -> Result<String> {
        self.calls
            .lock()
            .expect("calls")
            .push(json!([method, params]));
        Ok(json!({"result":{"turn":{"id":"accepted"}}}).to_string())
    }

    fn fork_from_rewind(
        &self,
        thread: &str,
        request: RewindForkRequest,
        _: Duration,
    ) -> Result<String> {
        self.forks
            .lock()
            .expect("forks")
            .push((thread.into(), request));
        Ok(json!({"result":{"thread":{"id":"fork","turns":[
            {"id":"kept","status":"completed","items":[
                {"type":"userMessage","id":"kept-user","content":[{"type":"text","text":"first"}]}
            ]}
        ]}}})
        .to_string())
    }

    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        bail!("no callback expected")
    }

    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("no callback expected")
    }

    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        bail!("no receiver expected")
    }

    fn stop(&mut self) {}
}

fn seed(kind: BackendKind) -> AppState {
    let mut app = AppState::new("root".into());
    app.session
        .set_claude_phase_for_test(carlos_core::session::claude::ClaudePhase::Ready);
    if kind == BackendKind::Codex {
        app.session.enable_native_history();
    }
    for (turn, id, text) in [("t1", "u1", "first"), ("t2", "u2", "second")] {
        user(&mut app, turn, id, text);
    }
    app.set_input_text("draft");
    app.enter_rewind_mode();
    app.set_input_text("edited");
    app
}

fn user(app: &mut AppState, turn: &str, id: &str, text: &str) {
    for method in ["item/started", "item/completed"] {
        handle_notification_line(
            app,
            &json!({"method":method,"params":{
            "threadId":"root","turnId":turn,"item":{"type":"userMessage","id":id,
            "content":[{"type":"text","text":text}]}}})
            .to_string(),
        );
    }
}

#[test]
fn each_backend_receives_the_original_anchor_before_the_edited_prompt() {
    for kind in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        let peer = Peer {
            kind,
            calls: Mutex::new(Vec::new()),
            forks: Mutex::new(Vec::new()),
        };
        let mut app = seed(kind);
        let selected = app
            .session
            .input_history()
            .prompt_id(1)
            .expect("second prompt");
        submit_rewind_turn_text(&peer, &mut app, selected, "edited".into());
        let native = (kind == BackendKind::Codex).then(|| RewindUserMessage {
            item_id: "u2".into(),
            text: "second".into(),
            position: 0,
        });
        assert_eq!(
            *peer.forks.lock().expect("forks"),
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
        if kind == BackendKind::Claude {
            assert!(peer.calls.lock().expect("calls").is_empty());
            assert!(crate::app::claude_edit::waiting(&app));
            app.exit_rewind_mode_restore();
            assert_eq!(app.input_text(), "draft");
        } else {
            assert_eq!(
                submitted_calls(&peer),
                vec![json!(["turn/start", {
                    "threadId":"fork","input":[{"type":"text","text":"edited","text_elements":[]}]
                }])]
            );
            assert!(!app.rewind_mode());
            app.set_input_text("later draft");
            assert!(app.navigate_input_history_up());
            assert_eq!(app.input_text(), "edited");
            assert!(app.navigate_input_history_up());
            assert_eq!(app.input_text(), "first");
            assert!(app.navigate_input_history_down());
            assert_eq!(app.input_text(), "edited");
            assert!(app.navigate_input_history_down());
            assert_eq!(app.input_text(), "later draft");
        }
    }
}

#[test]
fn invisible_native_users_keep_exact_positions_in_live_and_replaced_history() {
    for loaded in [false, true] {
        for hidden in [0, 1, 3] {
            let mut app = AppState::new("root".into());
            app.session.enable_native_history();
            let mut items = Vec::new();
            for index in 0..hidden {
                items.push(
                    json!({"type":"userMessage","id":format!("hidden-{index}"),"content":[]}),
                );
            }
            items.push(json!({"type":"userMessage","id":"visible","content":[{"type":"text","text":"visible"}]}));
            if loaded {
                let response = json!({"result":{"thread":{"id":"root","turns":[
                    {"id":"shared","status":"completed","items":items}
                ]}}});
                load_history_from_start_or_resume(&mut app, &response.to_string())
                    .expect("history");
            } else {
                for item in items {
                    for method in ["item/started", "item/completed", "item/completed"] {
                        handle_notification_line(
                            &mut app,
                            &json!({"method":method,"params":{
                            "threadId":"root","turnId":"shared","item":item}})
                            .to_string(),
                        );
                    }
                }
            }
            assert_eq!(app.input_history_len(), 1);
            app.enter_rewind_mode();
            assert_eq!(app.rewind_selected_turn_id(), Some("shared"));
            assert_eq!(
                app.rewind_native_request(),
                Some(RewindUserMessage {
                    item_id: "visible".into(),
                    text: "visible".into(),
                    position: hidden,
                })
            );
            let selected = app.rewind_selected_message_idx().expect("visible row");
            assert_eq!(
                app.session.messages()[selected].kind,
                if hidden == 0 {
                    MessageKind::Plain
                } else {
                    MessageKind::UserSteer
                }
            );
        }
    }
}

fn submitted_calls(peer: &Peer) -> Vec<Value> {
    let mut calls = peer.calls.lock().expect("calls").clone();
    if peer.kind == BackendKind::Codex {
        let token = calls[0][1]
            .as_object_mut()
            .expect("params")
            .remove("clientUserMessageId")
            .expect("exact native echo token");
        uuid::Uuid::parse_str(token.as_str().expect("UUID string")).expect("valid echo token");
    }
    calls
}
