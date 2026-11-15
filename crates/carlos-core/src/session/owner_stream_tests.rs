//! Preserve attached clients and reject unsafe cursor rebases when an owner is replaced.

use carlos_proto::events::Event;
use carlos_proto::ids::Sequence;

use super::OwnerStreamError;
use crate::session::event_tests::synchronize;
use crate::session::models::Role;
use crate::session::state::SessionState;

#[test]
fn a_replacement_owner_resets_an_existing_client_at_the_next_cursor() {
    let mut before = session();
    before.append_message(Role::User, "saved original");
    let mut client = before.publish().snapshot(100);
    let old_cursor = client.seq;
    let old_entry = client.entries[0].id.clone();
    let mut replacement = session();
    replacement.events.id = before.session_id().clone();
    replacement.append_message(Role::User, "saved original");
    replacement.append_message(Role::Assistant, "restored reply");
    replacement
        .continue_owner_stream(old_cursor)
        .expect("new owner has not published");
    let events = synchronize(&mut replacement, &mut client);
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].seq,
        old_cursor.checked_next().expect("next cursor")
    );
    assert!(matches!(events[0].event, Event::Reset { .. }));
    assert_eq!(client.info.id, before.session_id().clone());
    assert_ne!(client.entries[0].id, old_entry);
    assert_eq!(client.entries[1].text, "restored reply");
    replacement.append_message(Role::Assistant, "next live reply");
    synchronize(&mut replacement, &mut client);
    assert_eq!(client.entries[2].text, "next live reply");
}

#[test]
fn a_published_owner_cannot_rewind_or_rebase_its_live_stream() {
    let mut state = session();
    state.append_message(Role::User, "original");
    let mut client = state.publish().snapshot(100);
    assert_eq!(
        state.continue_owner_stream(Sequence::new(42)),
        Err(OwnerStreamError::AlreadyPublished)
    );
    state.append_message(Role::Assistant, "still live");
    synchronize(&mut state, &mut client);
    assert_eq!(client.entries[1].text, "still live");
}

#[test]
fn an_exhausted_cursor_fails_before_changing_an_unpublished_owner() {
    let mut state = session();
    let initial_reset = state.messages.journal.reset;
    assert_eq!(
        state.continue_owner_stream(Sequence::new(u64::MAX)),
        Err(OwnerStreamError::SequenceExhausted)
    );
    assert_eq!(state.events.seq, Sequence::default());
    assert_eq!(state.messages.journal.reset, initial_reset);
    state
        .continue_owner_stream(Sequence::new(10))
        .expect("failed read did not claim the stream");
    assert!(matches!(
        state.publish().records()[0].change,
        crate::session::events::Change::Reset { .. }
    ));
}

fn session() -> SessionState {
    SessionState::new("same-native".into(), Default::default(), false)
}
