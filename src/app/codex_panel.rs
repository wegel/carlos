//! Apply local task-panel changes after session-owned reads and controls.

use std::time::{Duration, Instant};

use super::codex_panel_state::PanelView;
use super::state::AppState;
use crate::backend::BackendClient;
use crate::session::codex::connection_state::ConnectionState;
use crate::session::codex::panel;
use crate::session::codex::panel_scope::{PanelScope, PanelUpdate};
use crate::session::codex::scope::CodexScope;

/// Borrow only the explicit local target while copying the native connection identity.
pub(super) fn scope<'a>(
    native: CodexScope,
    connection: &ConnectionState,
    view: &'a PanelView,
) -> PanelScope<'a> {
    PanelScope {
        native,
        generation: connection.generation(),
        rebuilding: connection.rebuilding(),
        visible: view.show,
        detail: view.detail,
        selected: view.selected.as_ref(),
    }
}

/// Poll native panel work without retaining client visibility in the session.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState) -> bool {
    let scope = scope(
        app.session.codex_scope(),
        &app.session.codex_connection(),
        &app.view.codex_panel,
    );
    let mut update = PanelUpdate::default();
    let changed = panel::tick(client, &mut app.session, scope, &mut update);
    apply(app, update);
    changed
}

/// Request a bounded native agent/process window.
pub(super) fn refresh(app: &mut AppState, older: bool) {
    let scope = scope(
        app.session.codex_scope(),
        &app.session.codex_connection(),
        &app.view.codex_panel,
    );
    panel::refresh(&mut app.session, scope, older);
}

/// Ask the session to read the explicitly selected child's recent native items.
pub(super) fn details(app: &mut AppState, older: bool) {
    let scope = scope(
        app.session.codex_scope(),
        &app.session.codex_connection(),
        &app.view.codex_panel,
    );
    let mut update = PanelUpdate::default();
    panel::details(&mut app.session, scope, &mut update, older);
    apply(app, update);
}

/// Stop only the exact native identity the user selected.
pub(super) fn stop(app: &mut AppState) {
    let scope = scope(
        app.session.codex_scope(),
        &app.session.codex_connection(),
        &app.view.codex_panel,
    );
    panel::stop(&mut app.session, scope);
}

/// Read the next native deadline without creating a timer for a hidden idle panel.
pub(super) fn wait(app: &AppState, now: Instant) -> Option<Duration> {
    panel::wait(
        &app.session,
        scope(
            app.session.codex_scope(),
            &app.session.codex_connection(),
            &app.view.codex_panel,
        ),
        now,
    )
}

/// Preserve selection identity while explicit arrows discard the previous child read.
pub(super) fn select(app: &mut AppState, down: bool) {
    app.view.codex_panel.select(down, app.session.codex_panel());
    app.session.clear_codex_panel_activity();
}

/// Apply exact session reports without replacing another local selection.
pub(super) fn apply(app: &mut AppState, update: PanelUpdate) {
    if let Some(selected) = update.selected {
        app.view.codex_panel.selected = Some(selected);
    }
    if update.detail {
        app.view.codex_panel.detail = true;
    }
    if update.reset_scroll {
        app.view.codex_panel.scroll = 0;
    }
}
