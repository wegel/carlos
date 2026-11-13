//! Apply native goal command effects without owning backend facts.

use serde_json::Value;

use super::codex_goals::apply;
use super::state::AppState;
use crate::session::codex::goal_receive;
use crate::session::codex::goals::GoalUpdate;

/// Route a native goal notification through the session before updating the view.
pub(super) fn observed(app: &mut AppState, method: &str, params: &Value) {
    let mut update = GoalUpdate::default();
    goal_receive::observed(&mut app.session, &mut update, method, params);
    apply(app, update);
}

/// Match root/epoch/request identity in the session and retain every unsent editor.
pub(super) fn reply(app: &mut AppState, params: &Value) -> bool {
    let scope = super::codex_queue::scope(app);
    let mut update = GoalUpdate::default();
    let handled = goal_receive::reply(&mut app.session, scope, &mut update, params);
    apply(app, update);
    handled
}
