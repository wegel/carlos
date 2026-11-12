//! Adopt backend forks only after the response and copied prefix are ready.

use anyhow::Context;

use crate::app::state::AppState;
use crate::app::Role;
use crate::protocol_params::{extract_result_object, parse_thread_id_from_start_or_resume};
use crate::session::models::EntryDraft;

/// Copy a retained prefix without adopting its ids into the future fork.
pub(super) fn snapshot_grok_rewind_prefix(
    app: &AppState,
    rewind_history_idx: usize,
) -> Vec<EntryDraft> {
    let cut = app
        .rewind_selected_message_idx()
        .or_else(|| nth_user_message_idx(app, rewind_history_idx))
        .unwrap_or(app.session.messages().len());
    let cut = cut.min(app.session.messages().len());
    app.session.messages()[..cut]
        .iter()
        .cloned()
        .map(Into::into)
        .collect()
}

fn nth_user_message_idx(app: &AppState, rewind_history_idx: usize) -> Option<usize> {
    app.session
        .messages()
        .iter()
        .enumerate()
        .filter(|(_, message)| message.role == Role::User)
        .nth(rewind_history_idx)
        .map(|(idx, _)| idx)
}

/// Adopt the copied Grok prefix with fresh ids after the fork succeeds.
pub(super) fn apply_grok_rewound_history(
    app: &mut AppState,
    response_line: &str,
    prefix: Vec<EntryDraft>,
) -> anyhow::Result<()> {
    let thread_id =
        parse_thread_id_from_start_or_resume(response_line).context("missing rewound thread id")?;
    app.reset_for_forked_thread(thread_id);
    for message in prefix {
        let record_user = message.role == Role::User && !message.text.is_empty();
        let text = message.text.clone();
        app.session.transcript_append(message);
        let idx = app.session.messages().len() - 1;
        if record_user {
            app.record_input_history(&text, Some(idx));
        }
        app.mark_transcript_dirty_from(idx);
    }
    Ok(())
}

/// Parse the complete Codex fork response before replacing the old transcript.
pub(super) fn apply_forked_thread_history(
    app: &mut AppState,
    response_line: &str,
) -> anyhow::Result<()> {
    let thread_id =
        parse_thread_id_from_start_or_resume(response_line).context("missing forked thread id")?;
    // Parse the full response before adopting the fork, preserving the old session on failure.
    let parsed = extract_result_object(response_line)?;
    let thread = parsed["result"]["thread"].clone();
    let previous = app.codex.item_turn.take();
    let mut draft = crate::app::item_history::history_draft::build(app, &thread);
    draft.set_conversation(&thread_id);
    app.codex.item_turn = previous;
    app.reset_for_forked_thread(thread_id);
    app.replace_history_draft(
        draft,
        crate::session::history_identity::HistoryMode::NewConversation,
    );
    app.read_thread_name(&thread);
    Ok(())
}

#[cfg(test)]
#[path = "../tests/entry_fork_tests.rs"]
mod identity_tests;
