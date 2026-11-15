//! Reuse native adapter startup and history readers while each owner handles startup questions.

use std::time::Duration;

use anyhow::Result;
use carlos_proto::session::Backend;

use super::{LaunchRequest, Launched};
use crate::backend::BackendClient;
use crate::claude_backend::{ClaudeClient, ClaudeLaunchMode};
use crate::codex_connection::CodexClient;
use crate::codex_snapshot::BootstrapHistory;
use crate::grok_backend::{GrokClient, GrokLaunchMode};
use crate::launch_cancel::LaunchCancellation;
use crate::protocol_params::{
    initialize_client, params_thread_resume, params_thread_start,
    parse_thread_id_from_start_or_resume, ThreadRuntimeSettings,
};
use crate::session::runtime_settings::RuntimeDefaults;
use crate::session::runtime_startup::{
    claude_runtime_defaults, grok_runtime_defaults, resolve_backend_runtime_settings,
};
use crate::session::state::SessionState;

pub(super) fn start(
    request: &LaunchRequest,
    cancel: &LaunchCancellation,
    preferences: RuntimeDefaults,
    summary_override: Option<String>,
) -> Result<Launched> {
    match request.backend {
        Backend::Codex => codex(request, cancel, preferences, summary_override),
        Backend::Claude => claude(request, cancel, preferences),
        Backend::Grok => grok(request, cancel, preferences),
    }
}

fn codex(
    request: &LaunchRequest,
    cancel: &LaunchCancellation,
    preferences: RuntimeDefaults,
    summary_override: Option<String>,
) -> Result<Launched> {
    let mut client = CodexClient::start(request.options.codex_socket.clone())?;
    let commands = client.capture_commands()?;
    cancel.register(move || commands.abort())?;
    initialize_client(&client)?;
    let (method, params) = if let Some(saved) = &request.restore {
        ("thread/resume", params_thread_resume(&saved.native_thread))
    } else {
        (
            "thread/start",
            params_thread_start(request.cwd.to_str().expect("checked cwd")),
        )
    };
    let response = client.call(method, params, Duration::from_secs(20))?;
    let thread = parse_thread_id_from_start_or_resume(&response)?;
    if let Some(saved) = &request.restore {
        anyhow::ensure!(
            thread == saved.native_thread,
            "native resume selected another thread"
        );
    }
    let mut session = SessionState::new(thread.clone(), preferences, false);
    session.enable_native_history();
    session.enable_codex();
    session.configure_codex_connection(
        client.connection_generation().expect("Codex generation"),
        request.options.codex_socket.clone(),
    );
    session.set_runtime_capabilities(true, true);
    session.prepare_codex_startup(request.restore.is_some(), summary_override);
    session.begin_initial_codex_rebuild();
    if request.restore.is_none() {
        session.events.launch = Some(carlos_proto::session::Availability::Starting);
    }
    // The owner starts before snapshot reads so replayed native approvals remain answerable.
    let history = if request.restore.is_some() {
        BootstrapHistory::ExistingThread
    } else {
        BootstrapHistory::NewThread
    };
    client.bootstrap(thread, response, history)?;
    let events = client.take_events_rx()?;
    Ok(Launched {
        session,
        backend: Box::new(client),
        events,
    })
}

fn claude(
    request: &LaunchRequest,
    cancel: &LaunchCancellation,
    preferences: RuntimeDefaults,
) -> Result<Launched> {
    let mode = request
        .restore
        .as_ref()
        .map(|saved| ClaudeLaunchMode::Resume(saved.native_thread.clone()))
        .unwrap_or(ClaudeLaunchMode::New);
    let context_window = request.options.native.claude_context_window.as_deref();
    let effective_context = context_window
        .map(str::to_owned)
        .or_else(|| std::env::var("CLAUDE_CODE_AUTO_COMPACT_WINDOW").ok());
    let mut client = ClaudeClient::start_with_context_window(&request.cwd, mode, context_window)?;
    let commands = client.capture_commands()?;
    cancel.register(move || commands.abort())?;
    let mut session = SessionState::new(client.session_id().into(), preferences, false);
    session.set_runtime_capabilities(true, false);
    session.enable_claude();
    session.configure_context_shell_override(effective_context.as_deref());
    seed_settings(&mut session, &claude_runtime_defaults());
    session.start_claude_settings();
    let events = client.take_events_rx()?;
    Ok(Launched {
        session,
        backend: Box::new(client),
        events,
    })
}

fn grok(
    request: &LaunchRequest,
    cancel: &LaunchCancellation,
    preferences: RuntimeDefaults,
) -> Result<Launched> {
    let mut client = GrokClient::start_cancellable(&request.cwd, cancel)?;
    let mode = request
        .restore
        .as_ref()
        .map(|saved| GrokLaunchMode::Resume(saved.native_thread.clone()))
        .unwrap_or(GrokLaunchMode::New);
    let native = request
        .restore
        .as_ref()
        .map_or("grok-starting", |saved| saved.native_thread.as_str());
    let mut session = SessionState::new(native.into(), preferences, false);
    session.set_runtime_capabilities(true, false);
    session.set_available_models(client.available_models());
    seed_settings(&mut session, &grok_runtime_defaults());
    session.begin_grok_start(request.restore.is_some());
    let events = client.take_events_rx()?;
    client.open_session_async(mode)?;
    Ok(Launched {
        session,
        backend: Box::new(client),
        events,
    })
}

fn seed_settings(session: &mut SessionState, fallback: &RuntimeDefaults) {
    let selected = resolve_backend_runtime_settings(
        ThreadRuntimeSettings::default(),
        session.runtime_settings().startup_preferences(),
        fallback,
        session.runtime_settings().available_models(),
        false,
    );
    session.set_runtime_settings(selected.model, selected.effort, selected.summary);
}

#[cfg(test)]
#[path = "startup_tests.rs"]
mod tests;
