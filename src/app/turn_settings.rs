//! Build per-prompt choices while letting supported Codex servers keep thread defaults authoritative.

use serde_json::Value;

use super::state::AppState;
use crate::backend::{BackendClient, BackendKind};
use crate::protocol_params::params_turn_start;

/// Omit cached Codex settings unless a precise missing-method reply requires compatibility fields.
pub(super) fn new_turn_params(client: &dyn BackendClient, app: &AppState, text: &str) -> Value {
    let (model, effort, summary) = if client.kind() == BackendKind::Codex && app.codex.enabled {
        if app.codex.settings.legacy_overrides {
            let choices = super::codex_settings_draft::choices(app);
            (
                choices["model"].as_str().map(str::to_string),
                choices["effort"].as_str().map(str::to_string),
                choices["summary"].as_str().map(str::to_string),
            )
        } else {
            (None, None, None)
        }
    } else {
        app.next_turn_runtime_settings()
    };
    let (model, effort) = if client.kind() == BackendKind::Claude {
        super::claude_models::prompt_choices(app, model, effort)
    } else {
        (model, effort)
    };
    params_turn_start(
        &app.thread_id,
        text,
        model.as_deref(),
        effort.as_deref(),
        summary.as_deref(),
    )
}
