//! Share question editing/rendering primitives while keeping backend reply formats separate.

use serde_json::Value;

use super::claude_question_state::NativeDraft;
use super::state::AppState;

/// Decide whether a pending request currently owns keyboard, paste and overlay focus.
pub(super) fn modal(app: &AppState) -> bool {
    if app.codex.enabled {
        app.approval.pending.is_some() && app.codex.callbacks.focused
    } else {
        app.approval.pending.is_some()
    }
}

/// Read the active native editor without coercing a Codex request ID to a string.
pub(super) fn draft(app: &AppState) -> Option<&NativeDraft> {
    let pending = app.approval.pending.as_ref()?;
    let id = &pending.request_id;
    if app.codex.enabled {
        Some(&app.codex.callbacks.get(id)?.draft)
    } else {
        app.claude.answers.drafts.get(id.as_str()?)
    }
}

/// Edit only the active callback's draft.
pub(super) fn draft_mut(app: &mut AppState) -> Option<&mut NativeDraft> {
    let pending = app.approval.pending.as_ref()?;
    let id = pending.request_id.clone();
    if app.codex.enabled {
        Some(&mut app.codex.callbacks.get_mut(&id)?.draft)
    } else {
        app.claude.answers.drafts.get_mut(id.as_str()?)
    }
}

/// Count requests without counting answered or resolved callbacks.
pub(super) fn count(app: &AppState) -> usize {
    if app.codex.enabled {
        app.codex.callbacks.entries.len()
    } else {
        app.claude.questions.len() + usize::from(app.approval.pending.is_some())
    }
}

/// Keep secret answers out of terminal echo and optional keystroke traces.
pub(super) fn secret_active(app: &AppState) -> bool {
    modal(app)
        && draft(app).is_some_and(|draft| {
            if draft.review {
                draft.fields.iter().any(|field| field.secret)
            } else {
                draft
                    .fields
                    .get(draft.current)
                    .is_some_and(|field| field.secret)
            }
        })
}

/// Display an exact native ID, including whether it was a string or a number.
pub(super) fn id_label(id: &Value) -> String {
    super::claude_question_state::safe_label(&id.to_string())
}

/// Keep visible questions ahead of hidden editor overlays when the user scrolls or clicks.
pub(super) fn mouse(
    app: &mut AppState,
    event: crossterm::event::MouseEvent,
) -> Option<super::input_events::TerminalEventResult> {
    if !modal(app) {
        return None;
    }
    if let Some(draft) = draft_mut(app) {
        let old = draft.viewport.scroll.get();
        let next = match event.kind {
            crossterm::event::MouseEventKind::ScrollUp => old.saturating_sub(3),
            crossterm::event::MouseEventKind::ScrollDown => {
                old.saturating_add(3).min(draft.viewport.max_scroll.get())
            }
            _ => old,
        };
        if next != old {
            draft.viewport.follow_focus.set(false);
            draft.viewport.scroll.set(next);
        }
        return Some(super::input_events::TerminalEventResult::Continue {
            needs_draw: next != old,
        });
    }
    Some(super::input_events::TerminalEventResult::Continue { needs_draw: false })
}
