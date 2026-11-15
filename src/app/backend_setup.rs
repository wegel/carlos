//! Backend launch and app setup helpers.

#[path = "backend_setup_codex_models.rs"]
mod backend_setup_codex_models;
#[path = "backend_setup_grok.rs"]
mod backend_setup_grok;
#[path = "backend_setup_launch.rs"]
mod backend_setup_launch;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use carlos_core::backend::{BackendClient, EventReceiver};
use carlos_core::protocol_params::{
    params_model_list, parse_model_list_page, ModelInfo, ThreadRuntimeSettings,
};
use carlos_core::session::models::ThreadSummary;
use carlos_core::session::runtime_settings::RuntimeDefaults;

use super::cli::{
    env_flag_enabled, resolve_initial_runtime_settings, styled_resume_hint_with_socket, Backend,
    CliOptions,
};
use super::input::run_conversation_tui;
use super::state::AppState;

pub(super) use self::backend_setup_codex_models::complete_codex_model_catalog;
pub(super) use self::backend_setup_grok::run_grok_backend;
pub(super) use self::backend_setup_launch::{run_claude_backend, run_codex_backend};

pub(super) use carlos_core::session::runtime_startup::{
    claude_runtime_defaults, codex_runtime_defaults, grok_runtime_defaults,
    resolve_backend_runtime_settings,
};

// --- Model Catalog ---

pub(super) fn fetch_model_catalog(client: &dyn BackendClient) -> Result<Vec<ModelInfo>> {
    let mut cursor = None;
    let mut out = Vec::new();
    loop {
        let resp = client.call(
            "model/list",
            params_model_list(cursor.as_deref()),
            Duration::from_secs(10),
        )?;
        let (mut page, next_cursor) = parse_model_list_page(&resp)?;
        out.append(&mut page);
        if next_cursor.is_none() {
            break;
        }
        cursor = next_cursor;
    }
    if out.is_empty() {
        return Ok(complete_codex_model_catalog(out));
    }
    Ok(out)
}

// --- App Helpers ---

pub(super) fn configure_app_common(
    app: &mut AppState,
    cwd: &str,
    opts: &CliOptions,
    runtime_settings: ThreadRuntimeSettings,
    persisted_defaults: &RuntimeDefaults,
    default_summary: Option<String>,
) {
    app.view.tool_output.mode = super::tool_output_preferences::load_mode();
    app.view.layout.themes = super::tool_output_preferences::load_themes();
    app.session.set_summary_cwd(PathBuf::from(cwd));
    app.configure_ralph_options(
        PathBuf::from(cwd),
        opts.ralph_prompt_path.clone(),
        opts.ralph_done_marker.clone(),
        opts.ralph_blocked_marker.clone(),
        opts.ralph_waiting_marker.clone(),
    );
    if env_flag_enabled("CARLOS_METRICS") {
        app.enable_perf_metrics();
    }
    let runtime_settings =
        resolve_initial_runtime_settings(runtime_settings, persisted_defaults, default_summary);
    app.set_runtime_settings(
        runtime_settings.model,
        runtime_settings.effort,
        runtime_settings.summary,
    );
    app.set_status("ready");
    configure_dictation(app, opts);
}

fn configure_dictation(app: &mut AppState, opts: &CliOptions) {
    app.view
        .configure_dictation_options(opts.dictation_profile.as_deref());
}

pub(super) fn finish_run(
    app: &mut AppState,
    backend: Backend,
    client: &dyn BackendClient,
    server_events_rx: EventReceiver,
) -> Result<()> {
    let actual = match client.kind() {
        carlos_core::backend::BackendKind::Codex => carlos_proto::session::Backend::Codex,
        carlos_core::backend::BackendKind::Claude => carlos_proto::session::Backend::Claude,
        carlos_core::backend::BackendKind::Grok => carlos_proto::session::Backend::Grok,
    };
    app.session.set_client_identity(actual, "local".into());
    let out = run_conversation_tui(client, app, server_events_rx);
    eprintln!(
        "{}",
        styled_resume_hint_with_socket(
            backend,
            app.session.thread_id(),
            app.session.codex_connection().socket().as_deref()
        )
    );
    if let Some(report) = app.perf_report() {
        eprintln!("{report}");
    }
    out
}

// --- Claude SDK sessions ---

/// Read the native SDK catalog, including Carlos programmatic sessions and worktrees.
pub(super) fn load_claude_thread_summaries(
    cwd_path: &Path,
    cwd: &str,
) -> Result<Vec<ThreadSummary>> {
    Ok(carlos_core::claude_backend::list_claude_sessions(cwd_path)?
        .iter()
        .filter_map(|session| {
            Some(ThreadSummary {
                id: session["sessionId"].as_str()?.to_string(),
                name: session["customTitle"].as_str().map(str::to_string),
                preview: session["summary"]
                    .as_str()
                    .unwrap_or("Claude session")
                    .to_string(),
                cwd: session["cwd"].as_str().unwrap_or(cwd).to_string(),
                created_at: session["createdAt"].as_i64().unwrap_or(0) / 1000,
                updated_at: session["lastModified"].as_i64().unwrap_or(0) / 1000,
            })
        })
        .collect())
}

#[cfg(test)]
pub(super) use self::backend_setup_launch::configure_new_codex_for_test;
