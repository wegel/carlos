//! Common source entries need no terminal fixture, layout cache or app object.

use super::models::{EntryDraft, Role};
use super::transcript::Transcript;

#[test]
fn source_entry_roles_need_no_view() {
    let mut transcript = Transcript::new();
    for role in [
        Role::User,
        Role::Assistant,
        Role::Commentary,
        Role::Reasoning,
        Role::ToolCall,
        Role::ToolOutput,
        Role::System,
    ] {
        let id = transcript.append(EntryDraft::plain(role, "source text"));
        let index = transcript.position(id).expect("live id");
        transcript.append_text(index, " delta", None);
        assert_eq!(
            transcript.entry(id).expect("entry").text,
            "source text delta"
        );
    }
}

#[test]
fn common_messages_need_no_view() {
    for backend in ["codex", "claude", "grok"] {
        let mut session = super::state::SessionState::new("root".into(), Default::default(), false);
        if backend == "codex" {
            session.enable_codex();
        }
        if backend == "claude" {
            session.enable_claude();
        }
        session.replace_codex_item_turn(Some("outside".into()));
        for line in common_lines() {
            session.handle_backend_line(
                &line.to_string(),
                &super::backend_test_support::context(&session),
            );
        }
        assert_eq!(session.messages().len(), 1);
        assert_eq!(session.messages()[0].role, Role::Assistant);
        assert_eq!(session.messages()[0].text, "hello world");
        assert_eq!(
            session.messages().position(session.messages()[0].id),
            Some(0)
        );
        assert!(session.active_turn_id().is_none());
        assert_eq!(session.context_usage().expect("token usage").used, 2500);
        assert_eq!(session.codex_item_turn().as_deref(), Some("outside"));
        assert_eq!(session.status(), "turn completed");
    }
}

pub(super) fn common_lines() -> [serde_json::Value; 6] {
    use serde_json::json;
    [
        json!({"method":"turn/started","params":{"threadId":"root","turn":{"id":"turn","status":"inProgress"}}}),
        json!({"method":"item/started","params":{"threadId":"root","turnId":"turn","item":{"id":"answer","type":"agentMessage"}}}),
        json!({"method":"item/agentMessage/delta","params":{"threadId":"root","turnId":"turn","itemId":"answer","delta":"hello "}}),
        json!({"method":"item/completed","params":{"threadId":"root","turnId":"turn","item":{"id":"answer","type":"agentMessage","text":"hello world"}}}),
        json!({"method":"thread/tokenUsage/updated","params":{"threadId":"root","tokenUsage":{"modelContextWindow":10000,"total":{"totalTokens":2500}}}}),
        json!({"method":"turn/completed","params":{"threadId":"root","turn":{"id":"turn","status":"completed"}}}),
    ]
}
