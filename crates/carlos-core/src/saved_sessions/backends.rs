//! Use native catalogue reads on temporary adapters owned entirely by a lifecycle worker.

use std::path::Path;
use std::time::Instant;

use anyhow::{ensure, Context, Result};
use carlos_proto::session::Backend;
use serde_json::{json, Value};

use super::{remaining, PageDirection, SavedPageRequest};
use crate::backend::BackendClient;
use crate::claude_backend::visit_saved_sessions;
use crate::codex_connection::CodexClient;
use crate::grok_backend::{visit_local_sessions, GrokClient};
use crate::launch_cancel::LaunchCancellation;
use crate::protocol_params::initialize_client;
use crate::session::models::ThreadSummary;
use crate::session::thread_list::parse_thread_list;

pub(super) fn visit(
    request: &SavedPageRequest,
    cwd: &Path,
    cancel: &LaunchCancellation,
    deadline: Instant,
    visitor: &mut dyn FnMut(ThreadSummary) -> bool,
) -> Result<()> {
    match request.backend {
        Backend::Codex => codex(request, cwd, cancel, deadline, visitor),
        Backend::Claude => visit_saved_sessions(cwd, cancel, deadline, &mut |row| {
            visitor(row);
        }),
        Backend::Grok => grok(request, cwd, cancel, deadline, visitor),
    }
}

fn codex(
    request: &SavedPageRequest,
    cwd: &Path,
    cancel: &LaunchCancellation,
    deadline: Instant,
    visitor: &mut dyn FnMut(ThreadSummary) -> bool,
) -> Result<()> {
    remaining(cancel, deadline)?;
    let client = CodexClient::start(request.codex_socket.clone())?;
    let commands = client.capture_commands()?;
    cancel.register(move || commands.abort())?;
    initialize_client(&client)?;
    let mut cursor = None::<String>;
    loop {
        let response = client.call(
            "thread/list",
            json!({"cwd":cwd,"limit":64,"cursor":cursor,"sortKey":"updated_at",
                "sortDirection":if matches!(request.direction, PageDirection::Oldest) { "asc" } else { "desc" }}),
            remaining(cancel, deadline)?,
        )?;
        let parsed: Value = serde_json::from_str(&response)?;
        if let Some(error) = parsed.get("error") {
            anyhow::bail!("native saved-session read failed: {error}");
        }
        for row in parse_thread_list(&response)? {
            if visitor(row) {
                return Ok(());
            }
        }
        let next = parsed["result"]["nextCursor"].as_str().map(str::to_owned);
        ensure!(
            next.is_none() || next != cursor,
            "native catalogue cursor did not advance"
        );
        let Some(next) = next else { break };
        ensure!(
            next.len() <= 16384,
            "native catalogue cursor exceeds 16 KiB"
        );
        cursor = Some(next);
    }
    Ok(())
}

fn grok(
    request: &SavedPageRequest,
    cwd: &Path,
    cancel: &LaunchCancellation,
    deadline: Instant,
    visitor: &mut dyn FnMut(ThreadSummary) -> bool,
) -> Result<()> {
    remaining(cancel, deadline)?;
    let client = GrokClient::start_cancellable(cwd, cancel)?;
    let listed = client.list_sessions();
    remaining(cancel, deadline)?;
    match listed {
        Ok(rows) if !rows.is_empty() || !request.allow_grok_fallback => {
            // The existing Grok continue rule takes the last native row on an updated-time tie.
            let rows: Vec<_> = if request.allow_grok_fallback {
                rows
            } else {
                rows.into_iter()
                    .max_by_key(|row| row.updated_at)
                    .into_iter()
                    .collect()
            };
            for row in rows {
                visitor(ThreadSummary {
                    id: row.session_id,
                    name: row.title,
                    preview: String::new(),
                    cwd: row.cwd,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                });
            }
            Ok(())
        }
        Err(error) if !request.allow_grok_fallback => Err(error).context("Grok saved sessions"),
        _ => visit_local_sessions(cwd, cancel, deadline, &mut |row| {
            visitor(row);
        }),
    }
}
