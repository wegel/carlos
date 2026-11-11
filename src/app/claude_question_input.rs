//! Routes native question keys to each callback's editor and native reply.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;
use std::time::Duration;

use super::claude_question_state::NativeDraft;
use super::input_events::TerminalEventResult;
use super::state::{AppState, ApprovalChoice};
use super::turn_submit::{interrupt_active_turn, respond_to_pending_approval};
use crate::backend::BackendClient;
use crate::clipboard::try_copy_clipboard;

/// Handle a native callback without borrowing the main message composer.
pub(super) fn handle_key(
    client: &dyn BackendClient,
    app: &mut AppState,
    key: KeyEvent,
    size: super::TerminalSize,
) -> TerminalEventResult {
    let result = TerminalEventResult::Continue { needs_draw: true };
    if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
        return TerminalEventResult::Quit;
    }
    if (size.width < 36 || size.height < 12)
        && key.code == KeyCode::Char('m')
        && key.modifiers == KeyModifiers::CONTROL
    {
        return result;
    }
    if control_key(client, app, key) {
        return result;
    }
    let editable = super::claude_question_state::can_edit_question(size);
    if let Some(choice) = answer_choice(app, key, editable) {
        respond_to_pending_approval(client, app, choice);
        return result;
    }
    if !editable {
        return result;
    }
    if app.codex.enabled {
        super::codex_callbacks::edited(app);
    }
    if let Some(draft) = super::native_questions::draft_mut(app) {
        if !page(draft, key) && !draft.fields.is_empty() {
            edit_question(draft, key);
        }
    }
    result
}

fn control_key(client: &dyn BackendClient, app: &mut AppState, key: KeyEvent) -> bool {
    if app.codex.enabled
        && key.code == KeyCode::Esc
        && super::codex_callbacks::hide_nonblocking(app)
    {
        return true;
    }
    match (key.code, key.modifiers) {
        (KeyCode::Char('m'), KeyModifiers::CONTROL) => app.toggle_model_settings(),
        (KeyCode::F(10), _) => interrupt_native(client, app),
        (KeyCode::F(4), _) => next_request(app),
        (KeyCode::Char('y'), KeyModifiers::CONTROL) => copy_browser_link(app),
        _ => return false,
    }
    true
}

fn interrupt_native(client: &dyn BackendClient, app: &mut AppState) {
    if app.codex.enabled {
        super::codex_reply::interrupt(client, app);
        return;
    }
    if app.active_turn_id.is_some() {
        interrupt_active_turn(client, app);
        return;
    }
    match client.call("turn/interrupt", json!({}), Duration::from_secs(3)) {
        Ok(_) => app.set_status("native interrupt requested"),
        Err(error) => app.set_status(format!("native interrupt failed: {error}")),
    }
}

fn answer_choice(app: &AppState, key: KeyEvent, editable: bool) -> Option<ApprovalChoice> {
    let fields_empty =
        super::native_questions::draft(app).is_some_and(|draft| draft.fields.is_empty());
    match (key.code, key.modifiers) {
        (KeyCode::Char('n'), KeyModifiers::ALT) => Some(ApprovalChoice::Decline),
        (KeyCode::Char('c'), KeyModifiers::ALT) => Some(ApprovalChoice::Cancel),
        (KeyCode::Char('s'), KeyModifiers::CONTROL) if editable => Some(ApprovalChoice::Accept),
        (KeyCode::Char('s'), mods)
            if editable
                && fields_empty
                && mods.is_empty()
                && app
                    .approval
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.can_accept_for_session) =>
        {
            Some(ApprovalChoice::AcceptForSession)
        }
        (KeyCode::Char('y' | 'a'), mods) if editable && fields_empty && mods.is_empty() => {
            Some(ApprovalChoice::Accept)
        }
        (KeyCode::Char('n'), mods) if fields_empty && mods.is_empty() => {
            Some(ApprovalChoice::Decline)
        }
        (KeyCode::Char('c'), mods) if fields_empty && mods.is_empty() => {
            Some(ApprovalChoice::Cancel)
        }
        _ => None,
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

/// Keep native pasted answers out of the main composer, including during startup.
pub(super) fn paste(app: &mut AppState, text: &str, size: super::TerminalSize) -> bool {
    if !super::native_questions::modal(app) {
        return false;
    }
    if app.codex.enabled {
        super::codex_callbacks::edited(app);
    }
    if let Some(draft) = super::native_questions::draft_mut(app) {
        if super::claude_question_state::can_edit_question(size) && !draft.review {
            if let Some(field) = draft.fields.get_mut(draft.current) {
                field.paste(text);
            }
        }
        return true;
    }
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
        if !field.required {
            field.omitted = !field.omitted;
        }
        return;
    }
    let count = field.options.len() + usize::from(field.other);
    match key.code {
        KeyCode::Up if count > 0 => field.cursor = (field.cursor + count - 1) % count,
        KeyCode::Down if count > 0 => field.cursor = (field.cursor + 1) % count,
        KeyCode::Enter if count > 0 && !field.multiple => {
            let was_text = field.use_text;
            field.select();
            if !field.use_text || was_text {
                draft.advance(false);
            }
        }
        KeyCode::Enter => draft.advance(false),
        KeyCode::Char(' ') if count > 0 && field.cursor < field.options.len() => field.select(),
        _ if count == 0 || (field.other && field.cursor == field.options.len()) => {
            if field.other {
                field.select();
            }
            field.edit(key);
        }
        _ => {}
    }
}

fn next_request(app: &mut AppState) {
    if app.codex.enabled {
        super::codex_callbacks::next(app);
        return;
    }
    if app.claude.questions.is_empty() {
        return;
    }
    if let Some(active) = app.approval.pending.take() {
        app.claude.questions.push_back(active);
    }
    app.approval.pending = app.claude.questions.pop_front();
    app.claude.confirm_allow = None;
}

fn copy_browser_link(app: &mut AppState) {
    if let Some(url) = app
        .approval
        .pending
        .as_ref()
        .and_then(|pending| pending.requested_permissions.as_ref())
        .and_then(|p| p["url"].as_str())
    {
        let _ = try_copy_clipboard(url);
        app.set_status("browser link copied; approve only if you consent to this server's request");
    }
}
