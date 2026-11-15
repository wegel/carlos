//! Preserve original native Claude edit history, guards and exact recovery ownership.

mod failures;
mod guards;

use carlos_proto::session::Backend;

use crate::session::claude::receive_update::ClaudeUpdate;
use crate::session::models::Role;
use crate::session::state::SessionState;

fn core() -> SessionState {
    let mut core = SessionState::new("original".into(), Default::default(), false);
    core.enable_claude();
    core.set_client_identity(Backend::Claude, "fixture".into());
    for index in 0..80 {
        core.append_message(Role::Assistant, format!("Retained row {index}"));
    }
    core
}

fn begin(core: &mut SessionState, text: &str) -> ClaudeUpdate {
    core.begin_claude_edit(text.into());
    let mut update = Default::default();
    core.retire_claude_queue(&mut update);
    update
}
