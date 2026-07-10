//! Backend launch and app setup helpers.

#[path = "backend_setup_codex_models.rs"]
mod backend_setup_codex_models;
#[path = "backend_setup_launch.rs"]
mod backend_setup_launch;

use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use anyhow::{Context, Result};

use super::cli::{
    env_flag_enabled, resolve_initial_runtime_settings, styled_resume_hint, Backend, CliOptions,
};
#[cfg(feature = "dictation")]
use super::dictation_state::DictationProfileState;
use super::input::run_conversation_tui;
use super::models::ThreadSummary;
use super::state::AppState;
use super::RuntimeDefaults;
use crate::backend::BackendClient;
use crate::claude_backend::claude_project_dir_name;
use crate::protocol_params::{
    params_model_list, parse_model_list_page, ModelInfo, ThreadRuntimeSettings,
};

pub(super) use self::backend_setup_codex_models::complete_codex_model_catalog;
pub(super) use self::backend_setup_launch::{run_claude_backend, run_codex_backend};

const CODEX_DEFAULT_MODEL: &str = "gpt-5.5";
const CODEX_DEFAULT_EFFORT: &str = "high";
const CODEX_DEFAULT_SUMMARY: &str = "concise";
const CLAUDE_DEFAULT_MODEL: &str = "opus";
const CLAUDE_DEFAULT_EFFORT: &str = "xhigh";

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
    Ok(complete_codex_model_catalog(out))
}

// --- App Helpers ---

pub(super) fn codex_runtime_defaults(default_summary: Option<String>) -> RuntimeDefaults {
    RuntimeDefaults {
        model: Some(CODEX_DEFAULT_MODEL.to_string()),
        effort: Some(CODEX_DEFAULT_EFFORT.to_string()),
        summary: default_summary.or_else(|| Some(CODEX_DEFAULT_SUMMARY.to_string())),
    }
}

pub(super) fn claude_runtime_defaults() -> RuntimeDefaults {
    RuntimeDefaults {
        model: Some(CLAUDE_DEFAULT_MODEL.to_string()),
        effort: Some(CLAUDE_DEFAULT_EFFORT.to_string()),
        summary: None,
    }
}

pub(super) fn resolve_backend_runtime_settings(
    runtime: ThreadRuntimeSettings,
    persisted_defaults: &RuntimeDefaults,
    fallback_defaults: &RuntimeDefaults,
    models: &[ModelInfo],
    supports_summary: bool,
) -> ThreadRuntimeSettings {
    let requested_model = runtime
        .model
        .or_else(|| persisted_defaults.model.clone())
        .or_else(|| fallback_defaults.model.clone());
    let model = choose_compatible_model(
        requested_model.as_deref(),
        fallback_defaults.model.as_deref(),
        models,
    );
    let model_info = model
        .as_deref()
        .and_then(|selected| find_model(models, selected));
    let model_uses_requested = requested_model.as_deref() == model.as_deref();
    let requested_effort = if model_uses_requested {
        runtime
            .effort
            .or_else(|| persisted_defaults.effort.clone())
            .or_else(|| fallback_defaults.effort.clone())
    } else {
        fallback_defaults.effort.clone()
    };
    let effort = choose_compatible_effort(
        requested_effort.as_deref(),
        fallback_defaults.effort.as_deref(),
        model_info,
    );
    let summary = supports_summary
        .then(|| {
            runtime
                .summary
                .or_else(|| persisted_defaults.summary.clone())
                .or_else(|| fallback_defaults.summary.clone())
        })
        .flatten();
    ThreadRuntimeSettings {
        model,
        effort,
        summary,
    }
}

fn find_model<'a>(models: &'a [ModelInfo], model: &str) -> Option<&'a ModelInfo> {
    models.iter().find(|candidate| candidate.model == model)
}

fn choose_compatible_model(
    requested: Option<&str>,
    fallback: Option<&str>,
    models: &[ModelInfo],
) -> Option<String> {
    if models.is_empty() {
        return requested.or(fallback).map(ToOwned::to_owned);
    }
    requested
        .and_then(|model| find_model(models, model))
        .or_else(|| fallback.and_then(|model| find_model(models, model)))
        .or_else(|| models.iter().find(|model| model.is_default))
        .or_else(|| models.first())
        .map(|model| model.model.clone())
}

fn choose_compatible_effort(
    requested: Option<&str>,
    fallback: Option<&str>,
    model: Option<&ModelInfo>,
) -> Option<String> {
    let Some(model) = model else {
        return requested.or(fallback).map(ToOwned::to_owned);
    };
    if model.supported_efforts.is_empty() {
        return requested.or(fallback).map(ToOwned::to_owned);
    }
    requested
        .and_then(|effort| find_effort(&model.supported_efforts, effort))
        .or_else(|| fallback.and_then(|effort| find_effort(&model.supported_efforts, effort)))
        .or_else(|| {
            model
                .default_effort
                .as_deref()
                .and_then(|effort| find_effort(&model.supported_efforts, effort))
        })
        .or_else(|| model.supported_efforts.first().cloned())
}

fn find_effort(efforts: &[String], effort: &str) -> Option<String> {
    efforts
        .iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(effort))
        .cloned()
}

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
