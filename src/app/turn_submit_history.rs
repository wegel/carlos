//! Drive fork fixtures through the same session adoption commands as accepted edits.

use super::super::state::AppState;
use carlos_core::session::models::EntryDraft;

pub(super) fn apply_grok_rewound_history(
    app: &mut AppState,
    response: &str,
    prefix: Vec<EntryDraft>,
) -> anyhow::Result<()> {
    let update = app.session.adopt_grok_fork(response, prefix)?;
    let _ = app.apply_claude_update(update);
    Ok(())
}

pub(super) fn apply_forked_thread_history(
    app: &mut AppState,
    response: &str,
) -> anyhow::Result<()> {
    let update = app.session.adopt_codex_fork(response)?;
    let _ = app.apply_claude_update(update);
    Ok(())
}

#[path = "../tests/entry_fork_tests.rs"]
mod identity_tests;
