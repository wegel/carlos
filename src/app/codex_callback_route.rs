//! Apply local editors after the session routes native callbacks and controls.

use serde_json::Value;

use crate::app::state::AppState;
use crate::session::actions::ServerRequestAction;
use crate::session::codex::callback_update::{CallbackEffect, CallbackUpdate};
use crate::session::codex::callbacks::CallbackKey;

/// Tell the shared parser whether a native callback event was consumed.
pub(in crate::app) enum CallbackRoute {
    Continue,
    Handled(Option<ServerRequestAction>),
}

/// Supply the explicit client target and consume one native result immediately.
pub(in crate::app) fn route(app: &mut AppState, method: &str, frame: &Value) -> CallbackRoute {
    let panel = crate::app::codex_panel::scope(
        app.session.codex_scope(),
        app.session.codex_connection(),
        &app.view.codex_panel,
    );
    let Some(update) =
        app.session
            .route_codex_callback(method, frame, panel, app.view.codex_callbacks.focused)
    else {
        return CallbackRoute::Continue;
    };
    CallbackRoute::Handled(apply(app, update))
}

/// Consume one report in the same order as the original immediate handler.
pub(in crate::app) fn apply(
    app: &mut AppState,
    update: CallbackUpdate,
) -> Option<ServerRequestAction> {
    match update.effect {
        CallbackEffect::None => {}
        CallbackEffect::Accepted { key, editor, focus } => {
            app.view
                .codex_callbacks
                .drafts
                .insert(key, crate::app::codex_questions::from_description(editor));
            if let Some(focused) = focus {
                app.view.codex_callbacks.focused = focused;
            }
            app.set_hint("Codex has a pending request; F4 answers it");
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
