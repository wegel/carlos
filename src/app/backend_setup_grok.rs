//! Grok backend launch: session pick, ACP session open, and history replay.

use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use anyhow::Result;

use super::{
    configure_app_common, finish_run, grok_runtime_defaults, resolve_backend_runtime_settings,
};
use crate::app::cli::CliOptions;
use crate::app::models::{Role, ThreadSummary};
use crate::app::notifications::{handle_server_message_line, load_history_from_start_or_resume};
use crate::app::state::AppState;
use crate::app::terminal_ui::pick_thread;
use crate::app::RuntimeDefaults;
use crate::backend::BackendClient;
use crate::grok_backend::{
    load_grok_thread_summaries, GrokClient, GrokLaunchMode, GrokSessionSummary,
};
use crate::protocol_params::ThreadRuntimeSettings;

// --- Launch ---

pub(in crate::app) fn run_grok_backend(
    opts: &CliOptions,
    cwd_path: &Path,
    cwd: &str,
    persisted_defaults: RuntimeDefaults,
    _default_summary: Option<String>,
) -> Result<()> {
    let mut client = GrokClient::start(cwd_path)?;
    let Some(launch_mode) = resolve_grok_launch_mode(opts, &client, cwd_path, cwd)? else {
        return Ok(());
    };
    let thread_id = client.open_session(&launch_mode)?;
    let server_events_rx = client.take_events_rx()?;
    let start_resp = client.synthetic_start_response(&thread_id);
    let mut app = AppState::new(thread_id);
    configure_grok_app(
        &mut app,
        &client,
        cwd,
        opts,
        &persisted_defaults,
        &start_resp,
    )?;
    apply_replayed_events(&mut app, &client, &server_events_rx, &launch_mode);
    finish_run(&mut app, opts.backend, &client, server_events_rx)
}

fn resolve_grok_launch_mode(
    opts: &CliOptions,
    client: &GrokClient,
    cwd_path: &Path,
    cwd: &str,
) -> Result<Option<GrokLaunchMode>> {
    if opts.mode_continue {
        return Ok(Some(GrokLaunchMode::Continue));
    }
    if let Some(session_id) = opts.resume_id.clone() {
        return Ok(Some(GrokLaunchMode::Resume(session_id)));
    }
    if opts.mode_resume {
        let list = list_grok_threads(client, cwd_path, cwd);
        let Some(session_id) = pick_thread(&list, false, |_| Ok(()))? else {
            return Ok(None);
        };
        return Ok(Some(GrokLaunchMode::Resume(session_id)));
    }
    Ok(Some(GrokLaunchMode::New))
}

fn list_grok_threads(client: &GrokClient, cwd_path: &Path, cwd: &str) -> Vec<ThreadSummary> {
    match client.list_sessions() {
        Ok(sessions) if !sessions.is_empty() => {
            return sessions.into_iter().map(to_thread_summary).collect();
        }
        _ => {}
    }
    load_grok_thread_summaries(cwd_path)
        .unwrap_or_default()
        .into_iter()
        .map(to_thread_summary)
        .filter(|thread| thread.cwd == cwd)
        .collect()
}

fn configure_grok_app(
    app: &mut AppState,
    client: &GrokClient,
    cwd: &str,
    opts: &CliOptions,
    persisted_defaults: &RuntimeDefaults,
    start_resp: &str,
) -> Result<()> {
    app.set_runtime_capabilities(true, false);
    let models = client.available_models();
    app.set_available_models(models.clone());
    let runtime_settings = resolve_backend_runtime_settings(
        ThreadRuntimeSettings {
            model: None,
            effort: None,
            summary: None,
        },
        persisted_defaults,
        &grok_runtime_defaults(),
        &models,
        false,
    );
    configure_app_common(
        app,
        cwd,
        opts,
        runtime_settings,
        &RuntimeDefaults::default(),
        None,
    );
    load_history_from_start_or_resume(app, start_resp)?;
    Ok(())
}

fn apply_replayed_events(
    app: &mut AppState,
    client: &GrokClient,
    server_events_rx: &Receiver<String>,
    launch_mode: &GrokLaunchMode,
) {
    let deadline = std::time::Instant::now() + Duration::from_millis(250);
    while std::time::Instant::now() < deadline {
        match server_events_rx.try_recv() {
            Ok(line) => {
                let _ = handle_server_message_line(app, &line);
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
        }
    }
    for line in client.take_replay_tail() {
        let _ = handle_server_message_line(app, &line);
    }
    if let Some(usage_line) = client.drain_replay_usage_line() {
        let _ = handle_server_message_line(app, &usage_line);
    }
    let is_resume = matches!(
        launch_mode,
        GrokLaunchMode::Resume(_) | GrokLaunchMode::Continue
    );
    if is_resume && app.messages.is_empty() {
        app.append_message(
            Role::System,
            "Grok session history could not be reconstructed from the agent replay".to_string(),
        );
    }
}

pub(super) fn to_thread_summary(session: GrokSessionSummary) -> ThreadSummary {
    ThreadSummary {
        preview: session
            .title
            .clone()
            .unwrap_or_else(|| session.session_id.clone()),
        name: session.title,
        id: session.session_id,
        cwd: session.cwd,
        created_at: session.created_at,
        updated_at: session.updated_at,
    }
}
