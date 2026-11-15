//! Publish accepted future/live settings through the native backend.

use serde_json::{json, Value};

use super::scope::CodexScope;
use crate::backend::BackendClient;
use crate::session::state::SessionState;

/// Return ordered local work without retaining an editor or calling client persistence.
#[derive(Default)]
pub struct SettingsUpdate {
    pub outcome: Option<(
        carlos_proto::submissions::ClientOperation,
        carlos_proto::submissions::OperationState,
    )>,
    pub accepted: bool,
    pub effects: Vec<SettingsEffect>,
}

/// Preserve the order of rejection, optional disk writes and visible notices.
pub enum SettingsEffect {
    ClearRejected,
    Rejected { desired: Value, message: String },
    Persist(Value),
    Notice(String),
}
/// Accept a plain settings request only after checking the current native catalog.
pub fn save_values(
    client: &dyn BackendClient,
    session: &mut SessionState,
    scope: CodexScope,
    update: &mut SettingsUpdate,
    desired: Value,
    persist: bool,
) {
    if let Err(error) = validate(session, &desired) {
        reject(
            session,
            update,
            &desired,
            json!({"code":-32602,"message":error}),
        );
        update.effects.push(SettingsEffect::Notice(format!(
            "Codex settings not sent: {error}"
        )));
        return;
    }
    update.accepted = true;
    update.effects.push(SettingsEffect::ClearRejected);
    session.codex_settings.flags.persist |= persist;
    session.codex_settings.flags.announce = true;
    session.codex_settings.deferred = Some(desired);
    tick(client, session, scope, update);
}

/// Keep native rejection facts in the session and offer the original attempted editor values.
pub fn reject(
    session: &mut SessionState,
    update: &mut SettingsUpdate,
    desired: &Value,
    error: Value,
) {
    if session.codex_settings.deferred.is_none() {
        let message = crate::session::text::safe_text(
            error["message"]
                .as_str()
                .unwrap_or("Codex rejected these choices"),
        );
        update.effects.push(SettingsEffect::Rejected {
            desired: desired.clone(),
            message,
        });
    }
    session.reject_runtime_settings(desired);
}
/// Send accepted deferred choices to future and live native scopes.
pub fn tick(
    client: &dyn BackendClient,
    session: &mut SessionState,
    scope: CodexScope,
    update: &mut SettingsUpdate,
) -> bool {
    super::settings_dispatch::dispatch(
        session,
        scope,
        update,
        None,
        |method, params, context, _| client.request_control_with_context(method, params, context),
    )
}

/// Send one checked remote intent through its captured connection and original write lifetime.
pub(crate) fn save_owned(
    commands: &crate::backend_commands::CommandHandle,
    session: &mut SessionState,
    scope: CodexScope,
    desired: Value,
    operation: carlos_proto::submissions::ClientOperation,
) -> SettingsUpdate {
    let mut update = SettingsUpdate::default();
    session.codex_settings.deferred = Some(desired);
    session.codex_settings.flags.announce = true;
    captured_dispatch(commands, session, scope, &mut update, Some(operation));
    update
}

/// Drive deferred startup work through the captured writer and the pending change's lifetime.
pub(crate) fn tick_captured(
    commands: &crate::backend_commands::CommandHandle,
    session: &mut SessionState,
    scope: CodexScope,
    update: &mut SettingsUpdate,
) -> bool {
    captured_dispatch(commands, session, scope, update, None)
}

fn captured_dispatch(
    commands: &crate::backend_commands::CommandHandle,
    session: &mut SessionState,
    scope: CodexScope,
    update: &mut SettingsUpdate,
    operation: Option<carlos_proto::submissions::ClientOperation>,
) -> bool {
    super::settings_dispatch::dispatch(
        session,
        scope,
        update,
        operation,
        |method, params, context, permission| {
            let guarded = commands.guarded(permission)?;
            std::ops::Deref::deref(&guarded).request_control_with_context(method, params, context)
        },
    )
}

/// Adopt newer authoritative choices before considering an older reply.
pub fn observed(session: &mut SessionState, params: &Value) {
    if params["threadId"] != *session.thread_id() || !params["threadSettings"].is_object() {
        return;
    }
    let settings = &params["threadSettings"];
    session.codex_settings.revision = session.codex_settings.revision.saturating_add(1);
    session.codex_settings.legacy_overrides = false;
    session.codex_settings.future = settings.clone();
    if session.codex_settings.deferred.is_none() {
        session.queue_runtime_settings(None, None, None);
    }
    session.apply_future_runtime_settings(settings);
}

/// Retire uncertain settings work and offer its attempted choices once.
pub fn closed(session: &mut SessionState, update: &mut SettingsUpdate) {
    let desired = session.codex_settings.deferred.take().or_else(|| {
        session
            .codex_settings
            .pending
            .take()
            .map(|change| change.desired)
    });
    if let Some(desired) = desired {
        reject(
            session,
            update,
            &desired,
            json!({"code":-32000,"message":"Codex closed before settings settled"}),
        );
    }
    session.codex_settings.pending = None;
}

/// Capture the choices used when this native turn started.
pub fn started(session: &mut SessionState, turn_id: &str) {
    let label = [
        session
            .runtime_settings()
            .current_model()
            .as_deref()
            .unwrap_or("model?"),
        session
            .runtime_settings()
            .current_effort()
            .as_deref()
            .unwrap_or(""),
        session
            .runtime_settings()
            .current_summary()
            .as_deref()
            .unwrap_or(""),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("/");
    session.codex_settings.active_before = Some((turn_id.into(), label));
}

/// Describe current-turn choices separately from future choices.
pub fn label(session: &SessionState) -> String {
    let future = session.runtime_settings_label();
    if let Some((turn_id, previous)) = &session.codex_settings.active_before {
        if session.active_turn_id().as_ref() == Some(turn_id) && *previous != future {
            return format!("{previous} → future {future}");
        }
    }
    future
}

/// Check plain model, effort and summary values against the native catalog.
pub fn validate(session: &SessionState, desired: &Value) -> Result<(), String> {
    if desired["summary"]
        .as_str()
        .is_some_and(|value| !["auto", "concise", "detailed", "none"].contains(&value))
    {
        return Err("Choose a supported summary setting".into());
    }
    if !session.runtime_settings().available_models().is_empty() {
        let model = session
            .runtime_settings()
            .available_models()
            .iter()
            .find(|model| {
                desired["model"]
                    .as_str()
                    .or(session.runtime_settings().current_model().as_deref())
                    == Some(model.model.as_str())
            })
            .ok_or("Choose a model advertised by Codex")?;
        if !model.supported_efforts.is_empty()
            && desired["effort"].as_str().is_some_and(|effort| {
                !model
                    .supported_efforts
                    .iter()
                    .any(|option| option == effort)
            })
        {
            return Err("Choose an effort advertised for the selected model".into());
        }
    }
    Ok(())
}
