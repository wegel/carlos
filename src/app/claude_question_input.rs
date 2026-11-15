//! Routes native question keys to each callback's editor and native reply.

use std::time::Duration;

use carlos_core::backend::BackendClient;
use carlos_core::session::approval_state::ApprovalChoice;
use carlos_core::session::approval_state::ApprovalQueue;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

use super::input_events::TerminalEventResult;
use super::state::AppState;
use super::turn_submit::{interrupt_active_turn, respond_to_pending_approval};
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
    if app.session.codex_scope().enabled() {
        super::codex_callbacks::edited(app);
    }
    if let Some(draft) = super::native_questions::draft_mut(app) {
        super::question_editor::key(draft, key);
    }
    result
}

fn control_key(client: &dyn BackendClient, app: &mut AppState, key: KeyEvent) -> bool {
    if app.session.codex_scope().enabled()
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
    if app.session.codex_scope().enabled() {
        super::codex_reply::interrupt(client, app);
        return;
    }
    if app.session.active_turn_id().is_some() {
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
                    .session
                    .approval()
                    .pending()
                    .is_some_and(|pending| pending.facts().can_accept_for_session) =>
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

/// Keep native pasted answers out of the main composer, including during startup.
pub(super) fn paste(app: &mut AppState, text: &str, size: super::TerminalSize) -> bool {
    if !super::native_questions::modal(app) {
        return false;
    }
    if app.session.codex_scope().enabled() {
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

fn next_request(app: &mut AppState) {
    if app.session.codex_scope().enabled() {
        super::codex_callbacks::next(app);
        return;
    }
    if app
        .session
        .approval()
        .queued(ApprovalQueue::Claude)
        .is_empty()
    {
        return;
    }
    let id = app.session.approval().queued(ApprovalQueue::Claude)[0].entry_id();
    app.session.focus_approval(id, ApprovalQueue::Claude);
    app.view.confirm_allow = None;
}

fn copy_browser_link(app: &mut AppState) {
    if let Some(url) = app
        .session
        .approval()
        .pending()
        .and_then(|pending| pending.facts().requested_permissions.as_ref())
        .and_then(|p| p["url"].as_str())
    {
        let _ = try_copy_clipboard(url);
        app.set_hint("browser link copied; approve only if you consent to this server's request");
    }
}
