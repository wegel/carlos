//! Keep native parser and transcript lifetime checks inside the session owner.

mod activity;
mod forks;
mod head_reference;
mod history_conversation;
mod history_identity;
mod identity;
mod identity_late;
mod identity_support;
mod idless_items;
mod item_sources;
mod lookup_differential;
mod lookup_extra;
mod names;
mod review_cursor;
mod review_history;

use super::actions::ServerRequestAction;
use super::backend_test_support::{action, context};
use super::state::SessionState;

fn session(root: &str) -> SessionState {
    SessionState::new(root.into(), Default::default(), false)
}

fn feed(session: &mut SessionState, line: &str) -> Option<ServerRequestAction> {
    let update = session.handle_backend_line(line, &context(session));
    let result = action(update);
    session.publish();
    result
}
