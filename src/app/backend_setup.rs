//! Backend launch and app setup helpers.

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
use super::models::{Role, ThreadSummary};
use super::notifications::{handle_server_message_line, load_history_from_start_or_resume, parse_thread_list};
use super::state::AppState;
use super::terminal_ui::{pick_thread, sort_threads_for_picker};
use super::RuntimeDefaults;
use crate::backend::BackendClient;
use crate::claude_backend::{
    claude_model_catalog, claude_project_dir_name, load_claude_local_history, ClaudeClient,
    ClaudeLocalHistory, ClaudeLaunchMode, CLAUDE_PENDING_THREAD_ID,
};
use crate::protocol::AppServerClient;
use crate::protocol_params::{
    extract_result_object, initialize_client, params_model_list, params_thread_archive,
    params_thread_list, params_thread_resume, params_thread_start, parse_model_list_page,
    parse_thread_id_from_start_or_resume, parse_thread_runtime_settings, ModelInfo,
    ThreadRuntimeSettings,
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
    Ok(out)
}
// --- App Helpers ---
pub(super) fn configure_app_common(app: &mut AppState, cwd: &str, opts: &CliOptions, runtime_settings: ThreadRuntimeSettings, persisted_defaults: &RuntimeDefaults, default_summary: Option<String>) {
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
            let profiles = config.profiles.values()
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
pub(super) fn claude_projects_root() -> Option<PathBuf> { env::var_os("HOME").map(PathBuf::from).map(|home| home.join(".claude").join("projects")) }
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
pub(super) fn load_claude_thread_summaries(cwd_path: &Path, cwd: &str) -> Result<Vec<ThreadSummary>> {
    let Some(projects_root) = claude_projects_root() else {
        return Ok(Vec::new());
    };
    let project_dir = projects_root.join(claude_project_dir_name(cwd_path));
    if !project_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for entry in fs::read_dir(&project_dir)
        .with_context(|| format!("failed to read Claude projects dir {}", project_dir.display()))?
    {
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
// --- Codex Backend ---
pub(super) fn run_codex_backend(opts: &CliOptions, _cwd_path: &Path, cwd: &str, persisted_defaults: RuntimeDefaults, default_summary: Option<String>) -> Result<()> {
    let mut client = AppServerClient::start()?;
    initialize_client(&client)?;
    let server_events_rx = client.take_events_rx()?;
    let Some((chosen_thread_id, start_resp)) = resolve_codex_thread(opts, &client, cwd)? else {
        return Ok(());
    };
    let mut app = AppState::new(chosen_thread_id);
    configure_codex_app(
        &mut app,
        &client,
        cwd,
        opts,
        &start_resp,
        &persisted_defaults,
        default_summary,
    )?;
    finish_run(&mut app, opts.backend, &client, server_events_rx)
}
pub(super) fn resolve_codex_thread(opts: &CliOptions, client: &AppServerClient, cwd: &str) -> Result<Option<(String, String)>> {
    if opts.mode_resume || opts.mode_continue {
        resolve_codex_resume_or_continue(opts, client, cwd)
    } else {
        let resp = client.call(
            "thread/start",
            params_thread_start(cwd),
            Duration::from_secs(20),
        )?;
        let thread_id = parse_thread_id_from_start_or_resume(&resp)?;
        Ok(Some((thread_id, resp)))
    }
}
pub(super) fn resolve_codex_resume_or_continue(opts: &CliOptions, client: &AppServerClient, cwd: &str) -> Result<Option<(String, String)>> {
    if let Some(rid) = opts.resume_id.as_deref() {
        return codex_resume_by_id(client, rid).map(Some);
    }
    let list_resp = client.call(
        "thread/list",
        params_thread_list(cwd),
        Duration::from_secs(15),
    )?;
    let list = parse_thread_list(&list_resp)?;
    let picked = if opts.mode_continue {
        sort_threads_for_picker(&list).into_iter().next().map(|t| t.id)
    } else {
        pick_thread(&list, true, |thread| {
            let resp = client.call(
                "thread/archive",
                params_thread_archive(&thread.id),
                Duration::from_secs(20),
            )?;
            extract_result_object(&resp).map(|_| ())
        })?
    };
    let Some(session_id) = picked else {
        return Ok(None);
    };
    codex_resume_by_id(client, &session_id).map(Some)
}
pub(super) fn codex_resume_by_id(client: &AppServerClient, session_id: &str) -> Result<(String, String)> {
    let resp = client.call(
        "thread/resume",
        params_thread_resume(session_id),
        Duration::from_secs(20),
    )?;
    let thread_id = parse_thread_id_from_start_or_resume(&resp)?;
    Ok((thread_id, resp))
}
pub(super) fn configure_codex_app(app: &mut AppState, client: &AppServerClient, cwd: &str, opts: &CliOptions, start_resp: &str, persisted_defaults: &RuntimeDefaults, default_summary: Option<String>) -> Result<()> {
    app.set_runtime_capabilities(true, true);
    if let Ok(models) = fetch_model_catalog(client) {
        app.set_available_models(models);
    }
    configure_app_common(
        app,
        cwd,
        opts,
        parse_thread_runtime_settings(start_resp)?,
        persisted_defaults,
        default_summary,
    );
    load_history_from_start_or_resume(app, start_resp)?;
    Ok(())
}
// --- Claude Backend ---
pub(super) fn run_claude_backend(opts: &CliOptions, cwd_path: &Path, cwd: &str, persisted_defaults: RuntimeDefaults, _default_summary: Option<String>) -> Result<()> {
    let Some(launch_mode) = resolve_claude_launch_mode(opts, cwd_path, cwd)? else {
        return Ok(());
    };
    let local_history = load_claude_local_history(cwd_path, &launch_mode)?;
    let (client, server_events_rx, start_resp) =
        start_claude_session(cwd_path, launch_mode, &local_history)?;
    let chosen_thread_id = parse_thread_id_from_start_or_resume(&start_resp)?;
    let mut app = AppState::new(chosen_thread_id);
    configure_claude_app(&mut app, cwd, opts, &persisted_defaults, &start_resp)?;
    apply_claude_local_history(&mut app, opts, &local_history)?;
    finish_run(&mut app, opts.backend, &client, server_events_rx)
}
pub(super) fn resolve_claude_launch_mode(opts: &CliOptions, cwd_path: &Path, cwd: &str) -> Result<Option<ClaudeLaunchMode>> {
    if opts.mode_continue {
        Ok(Some(ClaudeLaunchMode::Continue))
    } else if opts.mode_resume && opts.resume_id.is_none() {
        let list = load_claude_thread_summaries(cwd_path, cwd)?;
        let Some(session_id) = pick_thread(&list, false, |_| Ok(()))? else {
            return Ok(None);
        };
        Ok(Some(ClaudeLaunchMode::Resume(session_id)))
    } else if let Some(session_id) = opts.resume_id.clone() {
        Ok(Some(ClaudeLaunchMode::Resume(session_id)))
    } else {
        Ok(Some(ClaudeLaunchMode::New))
    }
}
pub(super) fn start_claude_session(cwd_path: &Path, launch_mode: ClaudeLaunchMode, local_history: &Option<ClaudeLocalHistory>) -> Result<(ClaudeClient, Receiver<String>, String)> {
    let initial_thread_id = match &launch_mode {
        ClaudeLaunchMode::Resume(session_id) => session_id.clone(),
        ClaudeLaunchMode::Continue => local_history
            .as_ref()
            .map(|history| history.session_id.clone())
            .unwrap_or_else(|| CLAUDE_PENDING_THREAD_ID.to_string()),
        ClaudeLaunchMode::New => CLAUDE_PENDING_THREAD_ID.to_string(),
    };
    let mut client = ClaudeClient::start(cwd_path, launch_mode)?;
    let server_events_rx = client.take_events_rx()?;
    let start_resp = client.synthetic_start_response(
        &initial_thread_id,
        local_history.as_ref().map(|history| &history.thread),
    );
    Ok((client, server_events_rx, start_resp))
}
pub(super) fn configure_claude_app(app: &mut AppState, cwd: &str, opts: &CliOptions, persisted_defaults: &RuntimeDefaults, start_resp: &str) -> Result<()> {
    app.set_runtime_capabilities(true, false);
    app.set_available_models(claude_model_catalog());
    configure_app_common(
        app,
        cwd,
        opts,
        ThreadRuntimeSettings {
            model: None,
            effort: None,
            summary: None,
        },
        &RuntimeDefaults::default(),
        None,
    );
    if persisted_defaults.model.is_some() || persisted_defaults.effort.is_some() {
        app.queue_runtime_settings(
            persisted_defaults.model.clone(),
            persisted_defaults.effort.clone(),
            None,
        );
    }
    load_history_from_start_or_resume(app, start_resp)?;
    Ok(())
}
pub(super) fn apply_claude_local_history(app: &mut AppState, opts: &CliOptions, local_history: &Option<ClaudeLocalHistory>) -> Result<()> {
    if let Some(usage) = local_history.as_ref().and_then(|history| history.context_usage) {
        app.context_usage = Some(super::context_usage::ContextUsage {
            used: usage.used,
            max: usage.max,
        });
    }
    if let Some(request_line) = local_history
        .as_ref()
        .and_then(|history| history.pending_approval_request.as_deref())
    {
        let _ = handle_server_message_line(app, request_line);
    }
    if (opts.mode_resume || opts.mode_continue)
        && local_history
            .as_ref()
            .map(|history| history.imported_item_count == 0)
            .unwrap_or(true)
    {
        app.append_message(
            Role::System,
            "Claude local transcript history could not be reconstructed from session storage"
                .to_string(),
        );
    }
    Ok(())
}
