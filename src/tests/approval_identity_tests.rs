//! Keep pending-card ids stable through duplicate delivery, focus, resolution and Claude clear.

use serde_json::{json, Value};

use crate::app::claude_callbacks::{cancel_callback, receive_callback};
use crate::app::codex_callbacks::{next, receive, retire};
use crate::app::state::AppState;
use crate::session::entry_id::EntryId;

fn active(app: &AppState) -> EntryId {
    app.approval
        .pending
        .as_ref()
        .expect("pending")
        .entry_id
        .expect("accepted card id")
}

#[test]
fn codex_card_ids_survive_duplicates_and_focus_then_retire_on_resolution() {
    for method in [
        "item/commandExecution/requestApproval",
        "item/fileChange/requestApproval",
        "item/tool/requestUserInput",
    ] {
        let mut app = AppState::new("thread".into());
        app.codex.enabled = true;
        let params = json!({"threadId":"thread","turnId":"turn","itemId":"item","command":"echo ok","questions":[{"id":"q","header":"q","question":"answer?","options":[]}]});
        let first = json!({"id":1,"params":params});
        receive(&mut app, method, &first);
        let id = active(&app);
        for _ in 0..3 {
            receive(&mut app, method, &first);
            assert_eq!(active(&app), id);
        }
        receive(&mut app, method, &json!({"id":2,"params":params}));
        next(&mut app);
        let second = active(&app);
        assert_ne!(id, second);
        next(&mut app);
        assert_eq!(active(&app), id);
        retire(&mut app, &json!(1));
        assert_eq!(active(&app), second);
        retire(&mut app, &json!(2));
        assert!(app.approval.pending.is_none());
        receive(&mut app, method, &json!({"id":3,"params":params}));
        assert!(![id, second].contains(&active(&app)));
        assert!(app.messages.is_empty());
    }
}

#[test]
fn claude_clear_keeps_process_card_ids_and_duplicate_clear_retires_rows_once() {
    for kind in ["permission", "elicitation"] {
        let mut app = AppState::new("thread".into());
        app.claude.enabled = true;
        let callback = |id| json!({"callbackId":id,"kind":kind,"payload":{"toolName":"Bash","input":{"command":"echo ok"},"requestedSchema":{"type":"object"}}});
        receive_callback(&mut app, &callback("a"));
        let a = active(&app);
        receive_callback(&mut app, &callback("b"));
        let b = app.claude.questions[0].entry_id.expect("queued card id");
        let old = app.append_message(crate::app::Role::User, "old");
        let old = app.messages[old].id;
        let clear = json!({"new_conversation_id":"reset"});
        crate::app::claude_reset::receive(&mut app, &clear);
        assert!(app.messages.entry(old).is_none());
        assert_eq!(active(&app), a);
        assert_eq!(app.claude.questions[0].entry_id, Some(b));
        app.append_message(crate::app::Role::System, "before native init");
        let new = app.messages[0].id;
        crate::app::claude_reset::receive(&mut app, &clear);
        assert_eq!(app.messages[0].id, new);
        assert_ne!(old, new);
        receive_callback(&mut app, &callback("a"));
        assert_eq!(active(&app), a);
        cancel_callback(&mut app, &Value::String("a".into()));
        assert_eq!(active(&app), b);
        cancel_callback(&mut app, &Value::String("b".into()));
        assert!(app.approval.pending.is_none());
        receive_callback(&mut app, &callback("a"));
        assert!(![a, b, old, new].contains(&active(&app)));
    }
}
