//! Read complete reconnect facts into one bounded slot without an oversized UI frame.

use std::time::{Duration, Instant};

use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

use crate::backend::BackendClient;
use crate::codex_snapshot_reads::SnapshotReader;
use crate::protocol::AppServerClient;
use crate::protocol_history::load_thread_history;
use crate::protocol_params::{extract_result_object, initialize_client, params_thread_resume};

/// Retain at most one complete read: 64 MiB history, 16 MiB queue, 8 MiB panel,
/// eight MiB catalog/runtime and 128 KiB goal, before UI replacement.
pub(crate) struct CodexSnapshot {
    pub(crate) thread: Value,
    pub(crate) runtime: String,
    pub(crate) catalog: Value,
    pub(crate) queue: Option<Value>,
    pub(crate) goal: Option<Value>,
    pub(crate) panel: Value,
}

/// Resume only the selected root, never a descendant, and never publish old Carlos settings.
pub(crate) fn read(
    client: &AppServerClient,
    thread: &str,
    deadline: Instant,
) -> Result<CodexSnapshot> {
    let reader = SnapshotReader::new(client, deadline);
    initialize_client(&reader)?;
    let runtime = reader.call(
        "thread/resume",
        params_thread_resume(thread),
        Duration::from_secs(20),
    )?;
    read_inner(reader, thread, runtime)
}

/// Reuse the initial handshake/resume while the UI handles replayed questions during history reads.
pub(crate) fn read_initialized(
    client: &AppServerClient,
    thread: &str,
    runtime: String,
) -> Result<CodexSnapshot> {
    read_inner(
        SnapshotReader::new(client, Instant::now() + Duration::from_secs(20)),
        thread,
        runtime,
    )
}

fn read_inner(reader: SnapshotReader<'_>, thread: &str, runtime: String) -> Result<CodexSnapshot> {
    let parsed = extract_result_object(&runtime)?;
    ensure!(
        parsed["result"]["thread"]["id"] == thread,
        "Codex resumed a different thread"
    );
    let history = load_thread_history(
        &reader,
        &runtime,
        reader.deadline.saturating_duration_since(Instant::now()),
    )?;
    let mut history = crate::app::reconnect_history(&history, thread)?;
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
    let panel = crate::app::reconnect_panel(&reader, thread)?;
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
    history["status"] = live["thread"]["status"].clone();
    if let Some(name) = live["thread"].get("name") {
        history["name"] = name.clone();
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
        thread: history,
        runtime,
        catalog,
        queue,
        goal,
        panel,
    })
}
