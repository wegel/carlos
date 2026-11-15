//! Apply native answer keys to a pure local draft without a session or backend client.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::claude_question_state::NativeDraft;

/// Share page, option, Unicode editor and field navigation behavior across terminal transports.
pub(super) fn key(draft: &mut NativeDraft, key: KeyEvent) {
    if !page(draft, key) && !draft.fields.is_empty() {
        edit_question(draft, key);
    }
}

fn page(draft: &mut NativeDraft, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::PageUp => draft.viewport.scroll.set(
            draft
                .viewport
                .scroll
                .get()
                .min(draft.viewport.max_scroll.get())
                .saturating_sub(5),
        ),
        KeyCode::PageDown => draft.viewport.scroll.set(
            draft
                .viewport
                .scroll
                .get()
                .saturating_add(5)
                .min(draft.viewport.max_scroll.get()),
        ),
        _ => return false,
    }
    draft.viewport.follow_focus.set(false);
    true
}

fn edit_question(draft: &mut NativeDraft, key: KeyEvent) {
    match key.code {
        KeyCode::Tab => {
            draft.advance(false);
            return;
        }
        KeyCode::BackTab => {
            draft.advance(true);
            return;
        }
        _ => {}
    }
    if draft.review {
        return;
    }
    draft.error = None;
    draft.viewport.scroll.set(0);
    draft.viewport.follow_focus.set(true);
    let Some(field) = draft.fields.get_mut(draft.current) else {
        return;
    };
    if key.code == KeyCode::Char('o') && key.modifiers == KeyModifiers::CONTROL {
        if !field.spec.required() {
            field.omitted = !field.omitted;
        }
        return;
    }
    let count = field.spec.options().len() + usize::from(field.spec.other());
    match key.code {
        KeyCode::Up if count > 0 => field.cursor = (field.cursor + count - 1) % count,
        KeyCode::Down if count > 0 => field.cursor = (field.cursor + 1) % count,
        KeyCode::Enter if count > 0 && !field.spec.multiple() => {
            let was_text = field.use_text;
            field.select();
            if !field.use_text || was_text {
                draft.advance(false);
            }
        }
        KeyCode::Enter => draft.advance(false),
        KeyCode::Char(' ') if count > 0 && field.cursor < field.spec.options().len() => {
            field.select()
        }
        _ if count == 0 || (field.spec.other() && field.cursor == field.spec.options().len()) => {
            if field.spec.other() {
                field.select();
            }
            field.edit(key);
        }
        _ => {}
    }
}
