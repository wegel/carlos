//! Apply local editors after the session routes native callbacks and controls.

use carlos_core::session::actions::ServerRequestAction;
use carlos_core::session::codex::callback_update::{CallbackEffect, CallbackUpdate};
use carlos_core::session::codex::callbacks::CallbackKey;
use serde_json::Value;

use crate::app::state::AppState;

/// Consume one report in the same order as the original immediate handler.
pub(in crate::app) fn apply(
    app: &mut AppState,
    update: CallbackUpdate,
) -> Option<ServerRequestAction> {
    match update.effect {
        CallbackEffect::None => {}
        CallbackEffect::ReplyFailed { id, error } => reply_failed(app, &id, error),
        CallbackEffect::Accepted {
            key,
            editor,
            card,
            blocking,
            active,
        } => {
            apply_accepted(app, key, editor, card, blocking, active);
        }
        CallbackEffect::Retired(retired) => super::apply_retired(app, retired),
        CallbackEffect::Checked { id, update } => {
            if let Some(draft) =
                CallbackKey::new(&id).and_then(|key| app.view.codex_callbacks.drafts.get_mut(&key))
            {
                if update.clear_notice {
                    draft.notice = None;
                }
                if let Some(error) = update.unsupported {
                    draft.unsupported = Some(error);
                }
                if let Some(error) = update.error {
                    draft.error = Some(error);
                }
            }
        }
        CallbackEffect::Items(items) => app.apply_item_changes(items),
        CallbackEffect::Goal(goal) => crate::app::codex_goals::apply(app, goal),
        CallbackEffect::Panel(panel) => crate::app::codex_panel::apply(app, panel),
        CallbackEffect::Queue(queue) => crate::app::codex_queue::apply(app, queue),
        CallbackEffect::Controls {
            interrupt,
            settings,
            queue,
        } => {
            if let Some((keys, error)) = interrupt {
                for key in keys {
                    if let Some(draft) = app.view.codex_callbacks.drafts.get_mut(&key) {
                        draft.notice = None;
                        draft.error = Some(error.clone());
                    }
                }
            }
            crate::app::codex_settings::apply(app, settings);
            crate::app::codex_queue::apply(app, queue);
        }
    }
    update.action
}

fn reply_failed(app: &mut AppState, id: &Value, error: String) {
    if let Some(draft) =
        CallbackKey::new(id).and_then(|key| app.view.codex_callbacks.drafts.get_mut(&key))
    {
        draft.error = Some(error.clone());
        draft.viewport.scroll.set(0);
    }
    app.set_hint(error);
}

fn apply_accepted(
    app: &mut AppState,
    key: CallbackKey,
    editor: carlos_core::session::codex::callback_questions::CallbackEditor,
    card: carlos_core::session::entry_id::EntryId,
    blocking: bool,
    active: bool,
) {
    app.view
        .codex_callbacks
        .drafts
        .insert(key, crate::app::codex_questions::from_description(editor));
    if active || blocking && !app.view.codex_callbacks.focused {
        app.session.focus_approval(
            card,
            carlos_core::session::approval_state::ApprovalQueue::Codex,
        );
        app.view.codex_callbacks.focused = blocking;
    }
    app.set_hint("Codex has a pending request; F4 answers it");
}
