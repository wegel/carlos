//! Keep original cursor-reset hints and immutable prompt identities at the Core record owner.

use super::session;
use crate::session::models::Role;
use crate::session::state::SessionState;

fn seed() -> SessionState {
    let mut state = session("root");
    for text in ["first", "second"] {
        let row = state.append_message(Role::User, text);
        assert!(state.record_prompt_history(text, Some(state.messages()[row].id)));
    }
    state
}

#[test]
fn recording_and_discarding_keep_the_original_reset_flags_and_prompt_identities() {
    for (operation, reset, length) in [
        ("duplicate", false, 2),
        ("empty", true, 2),
        ("append", true, 3),
        ("discard-bound", false, 2),
        ("discard-mismatch", false, 2),
    ] {
        let mut state = seed();
        let selected = state.input_history().prompt_id(1).expect("second prompt");
        let row = state.input_history().entry_id(1).expect("bound row");
        let changed = match operation {
            "duplicate" => state.record_prompt_history("second", Some(row)),
            "empty" => state.record_prompt_history("", None),
            "append" => state.record_prompt_history("pending", None),
            "discard-bound" => state.discard_pending_prompt("second"),
            "discard-mismatch" => state.discard_pending_prompt("different"),
            _ => unreachable!(),
        };
        assert_eq!(changed, reset, "{operation}");
        assert_eq!(state.input_history().history_len(), length, "{operation}");
        assert_eq!(state.input_history().prompt_id(1), Some(selected));
        assert_eq!(state.input_history().entry_id(1), Some(row));
    }
}

#[test]
fn native_binding_keeps_the_pending_prompt_id_and_discard_removes_only_the_unbound_tail() {
    let mut state = seed();
    assert!(state.record_prompt_history("pending", None));
    let pending = state.input_history().prompt_id(2).expect("pending record");
    assert_eq!(state.input_history().entry_id(2), None);
    let index = state.append_message(Role::User, "pending");
    let row = state.messages()[index].id;
    assert!(state.record_prompt_history("pending", Some(row)));
    assert_eq!(state.input_history().history_len(), 3);
    assert_eq!(state.input_history().prompt_id(2), Some(pending));
    assert_eq!(state.input_history().entry_id(2), Some(row));
    assert!(state.record_prompt_history("unsent", None));
    assert!(state.discard_pending_prompt("unsent"));
    assert_eq!(state.input_history().history_len(), 3);
    assert_eq!(state.input_history().prompt_id(2), Some(pending));
    assert_eq!(state.input_history().entry_id(2), Some(row));
}
