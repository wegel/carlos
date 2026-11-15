//! Keep issued card addresses stable through native duplicates and process-lifetime clears.

use serde_json::{json, Value};

use crate::session::approval_state::ApprovalQueue;
use crate::session::entry_id::EntryId;
use crate::session::models::Role;
use crate::session::state::SessionState;

fn active(core: &SessionState) -> EntryId {
    core.approval().pending().expect("pending").entry_id()
}

#[test]
fn codex_card_ids_survive_duplicates_and_focus_then_retire_on_resolution() {
    for method in [
        "item/commandExecution/requestApproval",
        "item/fileChange/requestApproval",
        "item/tool/requestUserInput",
    ] {
        let mut core = SessionState::new("thread".into(), Default::default(), false);
        core.set_codex_enabled_for_test(true);
        let params = json!({"threadId":"thread","turnId":"turn","itemId":"item","command":"echo ok","questions":[{"id":"q","header":"q","question":"answer?","options":[]}]});
        let first = json!({"id":1,"params":params});
        core.accept_codex_callback(method, &first);
        let id = active(&core);
        for _ in 0..3 {
            core.accept_codex_callback(method, &first);
            assert_eq!(active(&core), id);
        }
        core.accept_codex_callback(method, &json!({"id":2,"params":params}));
        let second = core.approval().queued(ApprovalQueue::Codex)[0].entry_id();
        core.focus_approval(second, ApprovalQueue::Codex);
        assert_ne!(id, second);
        core.focus_approval(id, ApprovalQueue::Codex);
        assert_eq!(active(&core), id);
        core.retire_codex_callback(&json!(1));
        assert_eq!(active(&core), second);
        core.retire_codex_callback(&json!(2));
        assert!(core.approval().pending().is_none());
        core.accept_codex_callback(method, &json!({"id":3,"params":params}));
        assert!(![id, second].contains(&active(&core)));
        assert!(core.messages().is_empty());
    }
}

#[test]
fn claude_clear_keeps_process_card_ids_and_duplicate_clear_retires_rows_once() {
    for kind in ["permission", "elicitation"] {
        let mut core = SessionState::new("thread".into(), Default::default(), false);
        core.set_claude_enabled_for_test(true);
        let callback = |id| json!({"callbackId":id,"kind":kind,"payload":{"toolName":"Bash","input":{"command":"echo ok"},"requestedSchema":{"type":"object"}}});
        core.receive_claude_callback(&callback("a"));
        let a = active(&core);
        core.receive_claude_callback(&callback("b"));
        let b = core.approval().queued(ApprovalQueue::Claude)[0].entry_id();
        let old = core.append_message(Role::User, "old");
        let old = core.messages()[old].id;
        let clear = json!({"new_conversation_id":"reset"});
        core.receive_claude_reset(&clear, &mut Default::default());
        assert!(core.messages().entry(old).is_none());
        assert_eq!(active(&core), a);
        assert_eq!(
            core.approval().queued(ApprovalQueue::Claude)[0].entry_id(),
            b
        );
        core.append_message(Role::System, "before native init");
        let new = core.messages()[0].id;
        core.receive_claude_reset(&clear, &mut Default::default());
        assert_eq!(core.messages()[0].id, new);
        assert_ne!(old, new);
        core.receive_claude_callback(&callback("a"));
        assert_eq!(active(&core), a);
        core.cancel_claude_callback(&Value::String("a".into()));
        assert_eq!(active(&core), b);
        core.cancel_claude_callback(&Value::String("b".into()));
        assert!(core.approval().pending().is_none());
        core.receive_claude_callback(&callback("a"));
        assert!(![a, b, old, new].contains(&active(&core)));
    }
}
