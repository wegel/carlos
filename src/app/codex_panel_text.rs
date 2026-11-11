//! Build terminal-safe panel labels and clearly bounded child details.

use serde_json::Value;

use super::claude_question_state::safe_text;
use super::codex_panel_state::PanelId;
use super::state::AppState;

/// Keep native state, control errors and ownership visible instead of relying on the status bar.
pub(super) fn body(app: &AppState) -> Vec<String> {
    let panel = &app.codex.panel;
    let mut lines = Vec::new();
    for (owner, error) in app
        .codex
        .task_status
        .errors
        .iter()
        .filter(|(owner, _)| super::task_badge::owns(app, owner))
    {
        lines.push(format!(
            "Background processes ({}): {}",
            safe_text(owner),
            safe_text(error)
        ));
    }
    if let Some(error) = &panel.control_error {
        lines.push(error.clone());
    }
    if let Some(error) = &panel.error {
        lines.push(error.clone());
    }
    if let Some(stop) = &panel.stop {
        lines.push(stop.message.clone());
    }
    if !panel.notice.is_empty() {
        lines.push(panel.notice.clone());
    }
    if app.codex.closed {
        lines.push("Codex is closed; panel controls are unavailable".into());
    }
    if !panel.selected_exists() {
        lines.push("No matching selected row. Arrows select; r refreshes latest.".into());
        return lines;
    }
    match panel.selected.as_ref().expect("selected row") {
        PanelId::Agent(id) => agent(app, id, &mut lines),
        PanelId::Process {
            thread,
            process,
            item,
        } => process_row(app, thread, process, item.as_deref(), &mut lines),
    }
    lines.into_iter().map(|line| safe_text(&line)).collect()
}

fn agent(app: &AppState, id: &str, lines: &mut Vec<String>) {
    let panel = &app.codex.panel;
    let agent = panel
        .agents
        .iter()
        .find(|agent| agent["id"] == id)
        .expect("selected agent");
    lines.extend([
        format!("Thread: {id}"),
        format!("Role: {}", text(&agent["agentRole"])),
        format!("Parent: {}", text(&agent["parentThreadId"])),
        format!("Native state: {}", agent["status"]),
        format!(
            "Direct input allowed: {} (this view is read-only)",
            text(&agent["canAcceptDirectInput"])
        ),
    ]);
    if let Some(turn) = agent.get("lastObservedTurn") {
        lines.push(format!(
            "Last observed turn: {} ({})",
            text(&turn["id"]),
            text(&turn["status"])
        ));
    }
    if let Some(report) = panel.reports.get(id) {
        lines.push(format!("Agent report ({}):", text(&report["status"])));
        lines.extend(
            bounded(&text(&report["message"]), 64 * 1024)
                .lines()
                .map(str::to_string),
        );
    }
    if panel.detail {
        activity(app, id, lines);
    } else {
        lines.push(format!(
            "{} agents / {} background processes; ↑↓ selects, Enter reads child activity",
            panel.agents.len(),
            panel.processes.len()
        ));
        lines.push(
            "Idle means no active native work; it does not prove the agent completed its task."
                .into(),
        );
    }
}

fn activity(app: &AppState, id: &str, lines: &mut Vec<String>) {
    let Some(activity) = app
        .codex
        .panel
        .activity
        .as_ref()
        .filter(|activity| activity.thread == id)
    else {
        return;
    };
    if let Some(error) = &activity.error {
        lines.push(error.clone());
    }
    lines.push(format!(
        "Read-only child activity: {} items retained (max128/8MiB)",
        activity.items.len()
    ));
    if activity.request.is_some() {
        lines.push("Reading child items…".into());
    }
    if activity.items.is_empty() && activity.request.is_none() {
        lines.push("No native item records returned. This does not mean no work ran; some native tools have no saved display record.".into());
    }
    if activity.more {
        lines.push("Older activity exists: l loads more, r reloads latest".into());
    }
    if activity.items.len() >= 128 {
        lines.push("128-item view limit reached; r reloads latest".into());
    }
    let mut remaining = 128 * 1024;
    for entry in activity.items.iter().rev() {
        if remaining == 0 {
            lines.push("Display limited to128KiB; native records remain retained".into());
            break;
        }
        let item = &entry["item"];
        lines.push(format!(
            "{} · {} · {}",
            text(&entry["turnId"]),
            text(&item["id"]),
            text(&item["type"])
        ));
        let content = content(item);
        let content = bounded(&content, remaining.min(32 * 1024));
        remaining = remaining.saturating_sub(content.len());
        lines.extend(content.lines().map(str::to_string));
    }
    if !activity.tail.is_empty() {
        lines.push("Live output since the last native item snapshot (last16KiB):".into());
        lines.extend(activity.tail.lines().map(str::to_string));
    }
}

fn process_row(
    app: &AppState,
    thread: &str,
    id: &str,
    item: Option<&str>,
    lines: &mut Vec<String>,
) {
    let row = app
        .codex
        .panel
        .processes
        .iter()
        .find(|row| {
            row.thread == thread
                && row.data["processId"] == id
                && row.data["itemId"].as_str() == item
        })
        .expect("selected process");
    lines.extend([
        format!("Process: {id}"),
        format!("Thread: {thread}"),
        format!("Item: {}", text(&row.data["itemId"])),
        format!("Command: {}", text(&row.data["command"])),
        format!("Directory: {}", text(&row.data["cwd"])),
        format!(
            "OS PID: {} · CPU: {} · RSS KiB: {}",
            text(&row.data["osPid"]),
            text(&row.data["cpuPercent"]),
            text(&row.data["rssKb"])
        ),
    ]);
    lines.push(
        "Present in the native background-process list; x terminates only this process.".into(),
    );
}

fn content(item: &Value) -> String {
    match item["type"].as_str() {
        Some("agentMessage" | "plan") => text(&item["text"]),
        Some("commandExecution") => format!(
            "{} [{}]\n{}",
            text(&item["command"]),
            text(&item["status"]),
            text(&item["aggregatedOutput"])
        ),
        _ => serde_json::to_string_pretty(item).unwrap_or_default(),
    }
}

fn text(value: &Value) -> String {
    if value.is_null() {
        "unavailable".into()
    } else {
        value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string())
    }
}

fn bounded(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.into();
    }
    let mut end = limit;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n[Display limited; complete native record retained]",
        &value[..end]
    )
}
