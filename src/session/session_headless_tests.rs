//! Common source entries need no terminal fixture, layout cache or app object.

use super::models::{EntryDraft, Role};
use super::transcript::Transcript;

#[test]
fn common_messages_need_no_view() {
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
