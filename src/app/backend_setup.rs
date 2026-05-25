//! Backend launch and app setup helpers.

use super::cli::{
    env_flag_enabled, resolve_initial_runtime_settings, styled_resume_hint, Backend, CliOptions,
};
#[cfg(feature = "dictation")]
use super::dictation_state::DictationProfileState;
use super::input::run_conversation_tui;
use super::models::{Role, ThreadSummary};
use super::notifications::{
    handle_server_message_line, load_history_from_start_or_resume, parse_thread_list,
};
use super::state::AppState;
use super::terminal_ui::{pick_thread, sort_threads_for_picker};
use super::RuntimeDefaults;
use crate::backend::BackendClient;
use crate::claude_backend::{
    claude_model_catalog, claude_project_dir_name, load_claude_local_history, ClaudeClient,
    ClaudeLaunchMode, ClaudeLocalHistory, CLAUDE_PENDING_THREAD_ID,
};
use crate::protocol::AppServerClient;
use crate::protocol_params::{
    extract_result_object, initialize_client, params_model_list, params_thread_archive,
    params_thread_list, params_thread_resume, params_thread_start, parse_model_list_page,
    parse_thread_id_from_start_or_resume, parse_thread_runtime_settings, ModelInfo,
    ThreadRuntimeSettings,
};
use anyhow::{Context, Result};
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;
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
    app.configure_ralph_options(
        PathBuf::from(cwd),
        opts.ralph_prompt_path.clone(),
        opts.ralph_done_marker.clone(),
        opts.ralph_blocked_marker.clone(),
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

#[cfg(feature = "dictation")]
fn configure_dictation(app: &mut AppState, opts: &CliOptions) {
    match crate::dictation::config::load_dictation_config(opts.dictation_profile.as_deref()) {
        Ok(config) => {
            let profiles = config
                .profiles
                .values()
                .map(|profile| DictationProfileState {
                    id: profile.id.clone(),
                    name: profile.name.clone(),
                    model_label: Some(profile.model.display().to_string()),
                    model_usable: profile.model_is_usable(),
                    model_path: Some(profile.model.clone()),
                    language: Some(profile.language.clone()),
                    vocabulary: profile.vocabulary.clone(),
                })
                .collect::<Vec<_>>();
            app.configure_dictation_profiles(profiles, &config.active_profile);
        }
        Err(err) => app.disable_dictation(format!("dictation unavailable: {err}")),
    }
}

#[cfg(not(feature = "dictation"))]
fn configure_dictation(app: &mut AppState, opts: &CliOptions) {
    let reason = if opts.dictation_profile.is_some() {
        "dictation unavailable: rebuild with --features dictation"
    } else {
        "dictation feature is not configured"
    };
    app.disable_dictation(reason);
}
pub(super) fn finish_run(
    app: &mut AppState,
    backend: Backend,
    client: &dyn BackendClient,
    server_events_rx: Receiver<String>,
) -> Result<()> {
    let out = run_conversation_tui(client, app, server_events_rx);
    eprintln!("{}", styled_resume_hint(backend, &app.thread_id));
    if let Some(report) = app.perf_report() {
        eprintln!("{report}");
    }
    out
}
// --- Claude History ---
pub(super) fn claude_projects_root() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".claude").join("projects"))
}
pub(super) fn file_mtime_secs(path: &Path) -> i64 {
    fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|mtime| mtime.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
pub(super) fn claude_preview_text_from_record(record: &serde_json::Value) -> Option<String> {
    let message = record.get("message")?.as_object()?;
    match message.get("content")? {
        serde_json::Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        serde_json::Value::Array(parts) => parts.iter().find_map(|part| {
            let text = match part.get("type").and_then(serde_json::Value::as_str) {
                Some("text") => part.get("text").and_then(serde_json::Value::as_str),
                Some("tool_result") => part.get("content").and_then(serde_json::Value::as_str),
                _ => None,
            }?;
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }),
        _ => None,
    }
}
pub(super) fn summarize_claude_session_file(path: &Path, cwd: &str) -> Option<ThreadSummary> {
    let session_id = path.file_stem()?.to_string_lossy().to_string();
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);
    let modified = file_mtime_secs(path);
    let mut name = None;
    let mut preview = None;
    let mut record_cwd = None;

    for line in reader.lines().filter_map(|line| line.ok()) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        if name.is_none() {
            name = record
                .get("customTitle")
                .or_else(|| record.get("agentName"))
                .or_else(|| record.get("slug"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
        }
        if record_cwd.is_none() {
            record_cwd = record
                .get("cwd")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
        }
        if preview.is_none() {
            preview = claude_preview_text_from_record(&record);
        }
    }

    Some(ThreadSummary {
        id: session_id.clone(),
        name,
        preview: preview.unwrap_or_else(|| session_id.clone()),
        cwd: record_cwd.unwrap_or_else(|| cwd.to_string()),
        created_at: modified,
        updated_at: modified,
    })
}
pub(super) fn load_claude_thread_summaries(
    cwd_path: &Path,
    cwd: &str,
) -> Result<Vec<ThreadSummary>> {
    let Some(projects_root) = claude_projects_root() else {
        return Ok(Vec::new());
    };
    let project_dir = projects_root.join(claude_project_dir_name(cwd_path));
    if !project_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for entry in fs::read_dir(&project_dir).with_context(|| {
        format!(
            "failed to read Claude projects dir {}",
            project_dir.display()
        )
    })? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        if let Some(summary) = summarize_claude_session_file(&path, cwd) {
            out.push(summary);
        }
    }
    Ok(out)
}

include!("backend_setup_launch.rs");
