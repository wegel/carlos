//! Apply local settings editor and persistence effects returned by the session.

use carlos_core::backend::BackendClient;
use carlos_core::session::codex::settings::{self, SettingsEffect, SettingsUpdate};
use serde_json::Value;

use super::state::AppState;

/// Submit the explicit runtime choices used by the settings fixtures.
#[cfg(test)]
pub(super) fn save(client: &dyn BackendClient, app: &mut AppState) {
    let desired = super::codex_settings_draft::choices(app);
    save_values(client, app, desired);
}

/// Pass chosen values and the unsent save preference to the session exactly once.
pub(super) fn save_values(client: &dyn BackendClient, app: &mut AppState, desired: Value) {
    let scope = super::codex_queue::scope(app);
    let mut update = SettingsUpdate::default();
    settings::save_values(
        client,
        &mut app.session,
        scope,
        &mut update,
        desired,
        app.view.codex_settings.persist_next,
    );
    if update.accepted {
        app.view.codex_settings.persist_next = false;
    }
    apply(app, update);
}

/// Send accepted deferred requests without rereading or changing the local editor.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState) -> bool {
    let scope = super::codex_queue::scope(app);
    let mut update = SettingsUpdate::default();
    let changed = settings::tick(client, &mut app.session, scope, &mut update);
    apply(app, update);
    changed
}

/// Capture settings when the native turn starts, without relabeling in-flight work.
#[cfg(test)]
pub(super) fn started(app: &mut AppState, turn: &str) {
    settings::started(&mut app.session, turn);
}

/// Read the existing current-turn/future settings label.
pub(super) fn label(app: &AppState) -> String {
    settings::label(&app.session)
}

/// Check plain values against the native catalog before changing the local editor.
pub(super) fn validate(app: &AppState, desired: &Value) -> Result<(), String> {
    settings::validate(&app.session, desired)
}

/// Preserve rejection, disk-write and transcript order from the original handler.
pub(super) fn apply(app: &mut AppState, update: SettingsUpdate) {
    for effect in update.effects {
        match effect {
            SettingsEffect::ClearRejected => {
                app.view.codex_settings.rejected = None;
                app.view.codex_settings.editor_error = None;
            }
            SettingsEffect::Rejected { desired, message } => {
                app.view.codex_settings.rejected = Some(desired);
                app.view.codex_settings.editor_error = Some(message);
            }
            SettingsEffect::Persist(desired) => super::codex_settings_draft::persist(app, &desired),
            SettingsEffect::Notice(message) => {
                app.append_message(carlos_core::session::models::Role::System, message);
            }
        }
    }
}
