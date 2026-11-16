//! Read reconnect facts and the newest complete history window without an oversized UI frame.

use std::time::{Duration, Instant};

use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

use crate::backend::BackendClient;
use crate::codex_snapshot_reads::SnapshotReader;
use crate::protocol::AppServerClient;
use crate::protocol_history::{newest_window, HistoryCursor, HistoryPage};
use crate::protocol_params::{extract_result_object, initialize_client, params_thread_resume};

/// Distinguish a newly created native thread from an existing thread with saved turns.
#[derive(Clone, Copy)]
pub enum BootstrapHistory {
    /// A thread/start response with no turns does not yet have saved history to read.
    NewThread,
    /// Resume and reconnect read the newest complete window and retain its older cursor.
    ExistingThread,
}

/// Retain at most one checked read: 64 MiB history window, 16 MiB queue, 8 MiB panel,
/// eight MiB catalog/runtime and 128 KiB goal, before UI replacement.
pub struct CodexSnapshot {
    pub thread: Value,
    pub(crate) older_history: Option<(HistoryCursor, Vec<String>)>,
    pub runtime: String,
    pub catalog: Value,
    pub queue: Option<Value>,
    pub goal: Option<Value>,
    pub panel: Value,
}

/// Resume only the selected root, never a descendant, and never publish old Carlos settings.
pub fn read(client: &AppServerClient, thread: &str, deadline: Instant) -> Result<CodexSnapshot> {
    let reader = SnapshotReader::new(client, deadline);
    initialize_client(&reader)?;
    let runtime = reader.call(
        "thread/resume",
        params_thread_resume(thread),
        Duration::from_secs(20),
    )?;
    read_inner(reader, thread, runtime, BootstrapHistory::ExistingThread)
}

/// Reuse the initial handshake while the owner handles replayed questions during history reads.
pub(crate) fn read_initialized(
    client: &AppServerClient,
    thread: &str,
    runtime: String,
    history: BootstrapHistory,
) -> Result<CodexSnapshot> {
    read_inner(
        SnapshotReader::new(client, Instant::now() + Duration::from_secs(20)),
        thread,
        runtime,
        history,
    )
}

fn read_inner(
    reader: SnapshotReader<'_>,
    thread: &str,
    runtime: String,
    history: BootstrapHistory,
) -> Result<CodexSnapshot> {
    let parsed = extract_result_object(&runtime)?;
    ensure!(
        parsed["result"]["thread"]["id"] == thread,
        "Codex resumed a different thread"
    );
    let mut history = initial_history(&reader, &parsed, &runtime, history)?;
    let catalog = reader.catalog()?;
    ensure!(
        catalog.to_string().len() + runtime.len() <= 8 * 1024 * 1024,
        "Codex reconnect runtime/catalog exceeds8 MiB"
    );
    let queue = reader.queue(thread)?;
    let goal = reader.optional("thread/goal/get", json!({"threadId":thread}))?;
    ensure!(
        goal.as_ref().map_or(0, |goal| goal.to_string().len()) <= 128 * 1024,
        "Codex reconnect goal exceeds128 KiB"
    );
    let panel = crate::session::codex::panel_snapshot::reconnect_panel(&reader, thread)?;
    let live = reader
        .optional(
            "thread/read",
            json!({"threadId":thread,"includeTurns":false}),
        )?
        .context("Codex metadata read is unavailable")?;
    ensure!(
        live["thread"]["id"] == thread,
        "Codex metadata belongs to another thread"
    );
    history.thread["status"] = live["thread"]["status"].clone();
    if let Some(name) = live["thread"].get("name") {
        history.thread["name"] = name.clone();
    }
    let mut runtime = parsed;
    for key in ["model", "reasoningEffort"] {
        runtime["result"][key] = live["thread"][key].clone();
    }
    runtime["result"]["summary"] = live["thread"]["summary"].clone();
    let runtime = runtime.to_string();
    ensure!(
        Instant::now() <= reader.deadline,
        "Codex reconnect exceeded20 seconds"
    );
    Ok(CodexSnapshot {
        thread: history.thread,
        older_history: history.older.map(|cursor| (cursor, history.visited)),
        runtime,
        catalog,
        queue,
        goal,
        panel,
    })
}

fn initial_history(
    reader: &SnapshotReader<'_>,
    parsed: &Value,
    runtime: &str,
    history: BootstrapHistory,
) -> Result<HistoryPage> {
    let fresh = &parsed["result"]["thread"];
    if matches!(history, BootstrapHistory::NewThread)
        && fresh["turns"].as_array().is_some_and(Vec::is_empty)
    {
        // Codex does not materialize an empty new thread until its first prompt.
        return Ok(HistoryPage {
            thread: fresh.clone(),
            older: None,
            visited: Vec::new(),
            read_bytes: 0,
        });
    }
    newest_window(
        reader,
        runtime,
        reader.deadline.saturating_duration_since(Instant::now()),
    )
}
