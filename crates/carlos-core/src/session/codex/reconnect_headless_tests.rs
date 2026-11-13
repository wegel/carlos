//! Reconnect facts, checked snapshots and retained charges need no terminal object.

use serde_json::json;

use super::connection_test_support::snapshot;
use super::reconnect_install::install;
use crate::session::models::Role;
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

#[test]
fn draining_held_native_frames_acknowledges_each_original_charge_once() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    let client = TestBackend::default();
    assert!(session.hold_codex_replay(json!({"method":"first"}), 17));
    assert!(session.hold_codex_replay(json!({"method":"second"}), 31));
    session.drain_codex_replay(&client);
    session.drain_codex_replay(&client);
    assert_eq!(*client.acknowledgements.lock().expect("acks"), [17, 31]);
    assert!(session.take_codex_replay_frame().is_none());
    assert!(!session.hold_codex_replay(json!({}), 12 * 1024 * 1024 + 1));
    session.drain_codex_replay(&client);
    assert_eq!(*client.acknowledgements.lock().expect("acks"), [17, 31]);
}

#[test]
fn invalid_snapshot_leaves_live_entries_and_runtime_facts_intact() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    let index = session.append_message(Role::Assistant, "old entry");
    let id = session.messages()[index].id;
    session.set_runtime_settings(Some("old-model".into()), None, None);
    session.close_codex();
    let mut invalid = snapshot();
    invalid.panel["verifiedOwners"] = json!(42);
    assert!(install(&mut session, invalid, None).is_err());
    assert_eq!(session.messages().len(), 1);
    assert_eq!(session.messages()[0].id, id);
    assert_eq!(session.messages()[0].text, "old entry");
    assert_eq!(
        session.runtime_settings().current_model().as_deref(),
        Some("old-model")
    );
    assert!(session.codex_scope().closed());
}

#[test]
fn complete_same_thread_snapshot_retains_ids_and_reopens_native_work() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    let first = snapshot();
    crate::session::item_history::replace_history_from_thread(&mut session, &first.thread)
        .expect("first history");
    let id = session.messages()[0].id;
    session.close_codex();
    let mut next = snapshot();
    next.thread["turns"][0]["items"][0]["text"] = json!("corrected");
    let update = install(&mut session, next, None).expect("valid snapshot");
    assert_eq!(session.messages()[0].id, id);
    assert_eq!(session.messages()[0].text, "corrected");
    assert!(!session.codex_scope().closed());
    assert_eq!(
        session.runtime_settings().current_model().as_deref(),
        Some("native-model")
    );
    assert!(update.history.history_replaced);
    assert!(matches!(
        session.publish().records()[0].change,
        crate::session::events::Change::Reset { .. }
    ));
}
