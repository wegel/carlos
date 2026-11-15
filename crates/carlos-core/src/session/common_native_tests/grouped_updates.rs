//! Report each native row touched when a grouped tool changes its label or role.

use super::session;
use crate::session::events::Change;
use crate::session::models::{MessageKind, Role};
use crate::session::state::SessionState;

#[test]
fn grouped_shell_updates_dirty_the_preceding_label_and_keep_following_spacing() {
    for role in [
        Role::Assistant,
        Role::ToolCall,
        Role::ToolOutput,
        Role::User,
    ] {
        let mut state = session("thread");
        state.append_message(Role::User, "question");
        for item in ["first", "shell"] {
            put(&mut state, item, Role::ToolCall, "→ Read a.rs");
        }
        state.append_message(Role::Assistant, "answer\n\nsecond paragraph");
        assert_eq!(state.messages()[1].text, "→ Read a.rs ×2");
        assert_eq!(state.messages()[2].text, "");
        state.publish();
        put(&mut state, "shell", Role::ToolCall, "→ Read a.rs");
        assert_dirty(&mut state, 1);
        assert_eq!(state.messages()[1].text, "→ Read a.rs ×3");
        put(&mut state, "shell", role, "changed shell\nsecond line");
        assert_dirty(&mut state, 2);
        state.upsert_agent_delta(&state.backend_entry_key("first"), "\nstreamed change");
        assert_dirty(&mut state, 1);
        assert_eq!(state.messages()[1].role, Role::ToolCall);
        assert_eq!(state.messages()[1].text, "→ Read a.rs ×3\nstreamed change");
    }
}

fn put(state: &mut SessionState, item: &str, role: Role, text: &str) {
    state.upsert_mapped_message(
        &state.backend_entry_key(item),
        role,
        text.into(),
        MessageKind::Plain,
        None,
    );
}

fn assert_dirty(state: &mut SessionState, expected: usize) {
    let batch = state.publish();
    let dirty = batch
        .records()
        .iter()
        .filter_map(|record| match record.change {
            Change::Added(id)
            | Change::Replaced(id)
            | Change::Facts(id)
            | Change::Text { entry: id, .. } => batch.position(id),
            Change::Reset { dirty_from } => {
                Some(dirty_from.and_then(|id| batch.position(id)).unwrap_or(0))
            }
            _ => None,
        })
        .min();
    assert_eq!(dirty, Some(expected));
}
