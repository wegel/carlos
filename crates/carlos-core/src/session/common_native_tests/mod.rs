//! Keep native parser and transcript lifetime checks inside the session owner.

mod activity;
mod codex_history;
mod codex_steer;
mod context_notifications;
mod diff_notifications;
mod forks;
mod grouped_updates;
mod head_reference;
mod history_conversation;
mod history_identity;
mod history_notifications;
mod identity;
mod identity_late;
mod identity_support;
mod idless_items;
mod input_classification;
mod item_sources;
mod lookup_differential;
mod lookup_extra;
mod names;
mod prompt_bindings;
mod review_cursor;
mod review_history;
mod rewind_anchors;
mod rtk_results;
mod shell_rendering;
mod summaries;
mod tool_command_views;
mod tool_fallbacks;
mod tool_overrides;
mod tool_results;
mod turn_lifecycle;

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
