//! Drive the actual settings reducer and captured prompt writer without a terminal view.

use serde_json::{json, Value};

use super::settings::{self, SettingsEffect, SettingsUpdate};
use crate::backend::{BackendKind, PromptOrigin};
use crate::session::backend_update::BackendChange;
use crate::session::codex::callback_update::CallbackEffect;
use crate::session::input_contract_test_support::rewind_peer;
use crate::session::prompt_commands::{PromptContext, PromptOptions};
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

pub(super) fn session() -> SessionState {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    session.enable_native_history();
    session.set_runtime_capabilities(true, true);
    session
}

pub(super) fn save(session: &mut SessionState, peer: &TestBackend) -> SettingsUpdate {
    let scope = session.codex_scope();
    let desired = session.codex_runtime_choices();
    let mut update = SettingsUpdate::default();
    settings::save_values(peer, session, scope, &mut update, desired, false);
    update
}

pub(super) fn tick(session: &mut SessionState, peer: &TestBackend) -> bool {
    settings::tick(
        peer,
        session,
        session.codex_scope(),
        &mut SettingsUpdate::default(),
    )
}

pub(super) fn reply(peer: &TestBackend, index: usize, response: Value) -> Value {
    let (method, params, context) = peer.calls.lock().expect("native controls")[index].clone();
    json!({"method":"carlos/codex/controlReply","params":{"requestMethod":method,
        "requestParams":params,"requestContext":context,"response":response}})
}

pub(super) fn receive(session: &mut SessionState, frame: Value) -> SettingsUpdate {
    let context = session.backend_context();
    let update = session.handle_backend_message(&frame, &context);
    if let BackendChange::Codex(update) = update.change {
        if let CallbackEffect::Controls { settings, .. } = update.effect {
            return settings;
        }
    }
    SettingsUpdate::default()
}

pub(super) fn notice(update: &SettingsUpdate, text: &str) -> bool {
    update
        .effects
        .iter()
        .any(|effect| matches!(effect, SettingsEffect::Notice(value) if value.contains(text)))
}

pub(super) fn send(session: &mut SessionState, text: &str) -> Value {
    let (commands, writes) = rewind_peer();
    let mut prepared = session
        .prepare_turn_text(
            BackendKind::Codex,
            text.into(),
            PromptOptions {
                record_history: true,
                force_start: false,
                origin: PromptOrigin::Human,
            },
            &PromptContext {
                operation: None,
                claude_queue: false,
            },
        )
        .expect("prepare ordinary native prompt");
    while let Some(work) = prepared.work {
        prepared = session
            .finish_backend_work(work.execute(&commands))
            .expect("settle exact prompt");
    }
    let writes = writes.lock().expect("captured native prompt");
    assert_eq!(writes.starts.len(), 1);
    writes.starts[0].clone()
}
