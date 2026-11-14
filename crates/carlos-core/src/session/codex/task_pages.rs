//! Validate complete targeted process pages before updating the task badge or F2 rows.

use std::time::{Duration, Instant};

use serde_json::Value;

use super::owners;
use crate::session::codex::panel_pages::valid_id;
use crate::session::codex::panel_scope::PanelScope;
use crate::session::codex::panel_state::{Process, MAX_BYTES, MAX_PROCESSES};
use crate::session::state::SessionState;
use crate::session::text::safe_label;

/// Accept one bounded page without exposing a partial process list.
pub(crate) fn accept_page(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    response: &Value,
) -> Result<(), String> {
    if let Some(error) = response.get("error") {
        return Err(format!(
            "Background process refresh failed: {}; F2 can retry",
            safe_label(error["message"].as_str().unwrap_or("native error"))
        ));
    }
    let read = session
        .codex_task_status
        .read
        .as_mut()
        .expect("targeted read");
    read.bytes = read.bytes.saturating_add(response.to_string().len());
    if read.bytes > MAX_BYTES || read.started.elapsed() >= Duration::from_secs(20) {
        return Err("Background process read exceeded 8 MiB or 20 seconds; F2 can retry".into());
    }
    for row in response["result"]["data"]
        .as_array()
        .ok_or("Malformed background process page")?
    {
        let id = valid_id(&row["processId"])?;
        valid_id(&row["itemId"])?;
        if read.items.len() >= MAX_PROCESSES
            || read.items.iter().any(|item| item.data["processId"] == id)
        {
            return Err("Background process page exceeded 512 records or repeated an ID".into());
        }
        read.items.push(Process {
            thread: read.request.params["threadId"]
                .as_str()
                .expect("target owner")
                .into(),
            data: row.clone(),
        });
    }
    let cursor = response["result"]["nextCursor"].clone();
    if !cursor.is_null() {
        let value = cursor.as_str().ok_or("Invalid background process cursor")?;
        if !read.cursors.insert(value.into()) {
            return Err("Repeated background process cursor".into());
        }
        read.request.params["cursor"] = cursor;
        read.sent = false;
    } else {
        publish(session, scope)?;
    }
    Ok(())
}

fn publish(session: &mut SessionState, scope: PanelScope<'_>) -> Result<(), String> {
    session.task_metadata_changed();
    let read = session
        .codex_task_status
        .read
        .take()
        .expect("complete targeted read");
    let owner = read.request.params["threadId"]
        .as_str()
        .expect("target owner");
    if !cache_fits(session, owner, &read.items) {
        session.codex_task_status.uncertain.insert(owner.into());
        session.codex_task_status.errors.insert(
            owner.into(),
            "Background process cache exceeded its 512-record/8 MiB bound".into(),
        );
        return Err("Background process cache exceeded its 512-record/8 MiB bound".into());
    }
    let nonempty = !read.items.is_empty();
    session
        .task_stops
        .reconcile_processes(Some(owner), &read.items);
    session.codex_panel.revision = session.codex_panel.revision.saturating_add(1);
    session
        .codex_panel
        .processes
        .retain(|row| row.thread != owner);
    session.codex_panel.processes.extend(read.items);
    session.codex_task_status.uncertain.remove(owner);
    session.codex_task_status.errors.remove(owner);
    if nonempty {
        session.codex_task_status.next = Some(Instant::now() + Duration::from_secs(3));
    } else if owners(session, scope).is_empty() {
        session.codex_task_status.next = None;
    }
    Ok(())
}

fn cache_fits(session: &SessionState, owner: &str, items: &[Process]) -> bool {
    let other: Vec<_> = session
        .codex_panel
        .processes
        .iter()
        .filter(|row| row.thread != owner)
        .collect();
    let agents: usize = session
        .codex_panel
        .agents
        .iter()
        .map(|agent| agent.to_string().len())
        .sum();
    let bytes = other
        .iter()
        .map(|row| row.data.to_string().len() + row.thread.len())
        .sum::<usize>()
        + items
            .iter()
            .map(|row| row.data.to_string().len() + row.thread.len())
            .sum::<usize>();
    other.len() + items.len() <= MAX_PROCESSES && agents + bytes <= MAX_BYTES
}
