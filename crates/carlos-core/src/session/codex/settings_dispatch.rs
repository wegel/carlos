//! Submit future and live settings through the same native tokens and one original write lifetime.

use std::sync::{atomic::AtomicBool, Arc};

use carlos_proto::submissions::{ClientOperation, OperationState};
use serde_json::{json, Value};

use super::scope::CodexScope;
use super::settings::{reject, SettingsEffect, SettingsUpdate};
use super::settings_state::{Change, SettingsRequest};
use crate::dispatch_guard::DispatchGuard;
use crate::session::state::SessionState;

pub(super) fn dispatch(
    session: &mut SessionState,
    scope: CodexScope,
    update: &mut SettingsUpdate,
    operation: Option<ClientOperation>,
    mut send: impl FnMut(&str, Value, Value, DispatchGuard) -> anyhow::Result<()>,
) -> bool {
    if !scope.enabled || scope.closed || session.codex_settings.pending.is_some() {
        return false;
    }
    let Some(desired) = session.codex_settings.deferred.take() else {
        return false;
    };
    let mut change = prepare(session, desired, operation);
    let permission = DispatchGuard::from_flag(change.request.live.clone());
    let mut params = change.desired.clone();
    params["threadId"] = json!(session.thread_id());
    let context =
        json!({"epoch":scope.epoch,"token":change.token,"operation":"settings","timeoutMs":15000});
    if let Err(error) = send(
        "thread/settings/update",
        params.clone(),
        context.clone(),
        permission.clone(),
    ) {
        change.thread = Some(format!(
            "Write not confirmed: {error}; read current settings before retrying"
        ));
        change.future_result = Some(OperationState::Uncertain {
            message: error.to_string(),
            recovery: None,
        });
        reject(
            session,
            update,
            &change.desired,
            json!({"code":-32000,"message":error.to_string()}),
        );
    }
    if let Some(turn_id) = &change.turn_id {
        params["turnId"] = json!(turn_id);
        if let Err(error) = send("turn/settings/update", params, context, permission) {
            change.turn = Some(format!("Failed: {error}"));
        }
    }
    if change.announce {
        update.effects.push(SettingsEffect::Notice("Codex settings pending: future turns and any live turn are updated separately. Already running model requests keep their captured settings.".into()));
    }
    session.codex_settings.pending = Some(change);
    super::settings_reply::finish(session, update);
    true
}

fn prepare(
    session: &mut SessionState,
    desired: Value,
    operation: Option<ClientOperation>,
) -> Change {
    // A metadata read started before this write cannot acknowledge or supersede its choices.
    if session.codex_connection.runtime_read.take().is_some() {
        session.codex_connection.runtime_refresh = true;
    }
    session.advance_codex_serial();
    let turn_id = session.active_turn_id().clone();
    Change {
        request: SettingsRequest {
            operation,
            live: Arc::new(AtomicBool::new(true)),
        },
        future_result: None,
        token: session.codex_serial(),
        desired,
        revision: session.codex_settings.revision,
        thread: None,
        turn: turn_id.is_none().then(|| "No live turn to update".into()),
        turn_id,
        persist: std::mem::take(&mut session.codex_settings.flags.persist),
        announce: std::mem::take(&mut session.codex_settings.flags.announce),
    }
}
