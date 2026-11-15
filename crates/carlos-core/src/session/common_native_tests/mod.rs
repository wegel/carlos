//! Keep native parser and transcript lifetime checks inside the session owner.

mod activity;
mod identity;
mod identity_late;
mod identity_support;
mod idless_items;

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
