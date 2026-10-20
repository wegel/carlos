//! Codex and Claude backend launch sequences: thread resume, session start, history hydration.

use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use anyhow::Result;

use super::{
    claude_runtime_defaults, codex_runtime_defaults, configure_app_common, fetch_model_catalog,
    finish_run, load_claude_thread_summaries, resolve_backend_runtime_settings,
};
use crate::app::cli::CliOptions;
use crate::app::codex_history::preferred_codex_thread;
use crate::app::item_history::replace_history_from_thread;
use crate::app::models::Role;
use crate::app::notifications::{
    handle_server_message_line, load_history_from_start_or_resume, parse_thread_list,
};
use crate::app::state::AppState;
use crate::app::terminal_ui::{pick_thread, sort_threads_for_picker};
use crate::app::RuntimeDefaults;
use crate::backend::BackendClient;
use crate::claude_backend::{
    claude_model_catalog, load_claude_local_history, ClaudeClient, ClaudeLaunchMode,
    ClaudeLocalHistory, CLAUDE_PENDING_THREAD_ID,
};
use crate::protocol::AppServerClient;
use crate::protocol_history::load_thread_history;
use crate::protocol_params::{
    extract_result_object, initialize_client, params_thread_archive, params_thread_list,
    params_thread_read, params_thread_resume, params_thread_start,
    parse_thread_id_from_start_or_resume, parse_thread_runtime_settings, ThreadRuntimeSettings,
};

// --- Codex Backend ---

/// Responses needed to resume one Codex thread and rebuild its transcript.
pub(super) struct CodexThreadLaunch {
    thread_id: String,
    runtime_response: String,
    history_response: String,
}

pub(in crate::app) fn run_codex_backend(
    opts: &CliOptions,
    _cwd_path: &Path,
    cwd: &str,
    persisted_defaults: RuntimeDefaults,
    default_summary: Option<String>,
) -> Result<()> {
    let mut client = AppServerClient::start()?;
    initialize_client(&client)?;
    let server_events_rx = client.take_events_rx()?;
    let Some(launch) = resolve_codex_thread(opts, &client, cwd)? else {
        return Ok(());
    };
    let mut app = AppState::new(launch.thread_id.clone());
    configure_codex_app(
        &mut app,
        &client,
        cwd,
        opts,
        &launch,
        &persisted_defaults,
        default_summary,
    )?;
    finish_run(&mut app, opts.backend, &client, server_events_rx)
}

pub(super) fn resolve_codex_thread(
    opts: &CliOptions,
    client: &AppServerClient,
    cwd: &str,
) -> Result<Option<CodexThreadLaunch>> {
    if opts.mode_resume || opts.mode_continue {
        resolve_codex_resume_or_continue(opts, client, cwd)
    } else {
        let resp = client.call(
            "thread/start",
            params_thread_start(cwd),
            Duration::from_secs(20),
        )?;
        let thread_id = parse_thread_id_from_start_or_resume(&resp)?;
        Ok(Some(CodexThreadLaunch {
            thread_id,
            runtime_response: resp.clone(),
            history_response: resp,
        }))
    }
}

pub(super) fn resolve_codex_resume_or_continue(
    opts: &CliOptions,
    client: &AppServerClient,
    cwd: &str,
) -> Result<Option<CodexThreadLaunch>> {
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
        sort_threads_for_picker(&list)
            .into_iter()
            .next()
            .map(|t| t.id)
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

pub(super) fn codex_resume_by_id(
    client: &AppServerClient,
    session_id: &str,
) -> Result<CodexThreadLaunch> {
    let history_resp = client
        .call(
            "thread/read",
            params_thread_read(session_id),
            Duration::from_secs(20),
        )
        .ok()
        .filter(|response| {
            parse_thread_id_from_start_or_resume(response)
                .is_ok_and(|thread_id| thread_id == session_id)
        });
    let resp = client.call(
        "thread/resume",
        params_thread_resume(session_id),
        Duration::from_secs(20),
    )?;
    let thread_id = parse_thread_id_from_start_or_resume(&resp)?;
    let history_resp = history_resp.unwrap_or_else(|| resp.clone());
    let history_resp = load_thread_history(client, &history_resp, Duration::from_secs(20))?;
    Ok(CodexThreadLaunch {
        thread_id,
        runtime_response: resp,
        history_response: history_resp,
    })
}

pub(super) fn configure_codex_app(
    app: &mut AppState,
    client: &AppServerClient,
    cwd: &str,
    opts: &CliOptions,
    launch: &CodexThreadLaunch,
    persisted_defaults: &RuntimeDefaults,
    default_summary: Option<String>,
) -> Result<()> {
    app.set_runtime_capabilities(true, true);
    let models = fetch_model_catalog(client).unwrap_or_default();
    app.set_available_models(models.clone());
    let fallback_defaults = codex_runtime_defaults(default_summary);
    let runtime_settings = resolve_backend_runtime_settings(
        parse_thread_runtime_settings(&launch.runtime_response)?,
        persisted_defaults,
        &fallback_defaults,
        &models,
        true,
    );
    configure_app_common(
        app,
        cwd,
        opts,
        runtime_settings,
        &RuntimeDefaults::default(),
        None,
    );
    let history = preferred_codex_thread(&launch.history_response, &app.thread_id)?;
    replace_history_from_thread(app, &history);
    Ok(())
}

// --- Claude Backend ---

pub(in crate::app) fn run_claude_backend(
    opts: &CliOptions,
    cwd_path: &Path,
    cwd: &str,
    persisted_defaults: RuntimeDefaults,
    _default_summary: Option<String>,
) -> Result<()> {
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

pub(super) fn resolve_claude_launch_mode(
    opts: &CliOptions,
    cwd_path: &Path,
    cwd: &str,
) -> Result<Option<ClaudeLaunchMode>> {
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

pub(super) fn start_claude_session(
    cwd_path: &Path,
    launch_mode: ClaudeLaunchMode,
    local_history: &Option<ClaudeLocalHistory>,
) -> Result<(ClaudeClient, Receiver<String>, String)> {
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

pub(super) fn configure_claude_app(
    app: &mut AppState,
    cwd: &str,
    opts: &CliOptions,
    persisted_defaults: &RuntimeDefaults,
    start_resp: &str,
) -> Result<()> {
    app.set_runtime_capabilities(true, false);
    let models = claude_model_catalog();
    app.set_available_models(models.clone());
    let fallback_defaults = claude_runtime_defaults();
    let runtime_settings = resolve_backend_runtime_settings(
        ThreadRuntimeSettings {
            model: None,
            effort: None,
            summary: None,
        },
        persisted_defaults,
        &fallback_defaults,
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

pub(super) fn apply_claude_local_history(
    app: &mut AppState,
    opts: &CliOptions,
    local_history: &Option<ClaudeLocalHistory>,
) -> Result<()> {
    if let Some(usage) = local_history
        .as_ref()
        .and_then(|history| history.context_usage)
    {
        app.context_usage = Some(crate::app::context_usage::ContextUsage {
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
