//! Build terminal-safe panel labels and clearly bounded child details.

use carlos_core::session::codex::panel_state::PanelId;
use carlos_core::session::text::safe_text;
use serde_json::Value;

use super::state::AppState;

/// Keep native state, control errors and ownership visible instead of relying on the status bar.
pub(super) fn body(app: &AppState) -> Vec<String> {
    let panel = &app.view.codex_panel;
    let facts = app.session.codex_panel();
    let mut lines = Vec::new();
    for (owner, error) in app
        .session
        .codex_task_status()
        .errors()
        .iter()
        .filter(|(owner, _)| super::task_badge::owns(app, owner))
    {
        lines.push(format!(
            "Background processes ({}): {}",
            safe_text(owner),
            safe_text(error)
        ));
    }
    if let Some(error) = facts.control_error() {
        lines.push(error.to_string());
    }
    if let Some(error) = facts.error() {
        lines.push(error.to_string());
    }
    if let Some(stop) = &facts.stop() {
        lines.push(stop.message().to_string());
    }
    if !facts.notice().is_empty() {
        lines.push(facts.notice().to_string());
    }
    if app.session.codex_scope().closed() {
        lines.push("Codex is closed; panel controls are unavailable".into());
    }
    if !facts.contains_target(panel.selected.as_ref()) {
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
    let panel = &app.view.codex_panel;
    let facts = app.session.codex_panel();
    let agent = facts
        .agents()
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
    if let Some(report) = facts.reports().get(id) {
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
            facts.agents().len(),
            facts.processes().len()
        ));
        lines.push(
            "Idle means no active native work; it does not prove the agent completed its task."
                .into(),
        );
    }
}

fn activity(app: &AppState, id: &str, lines: &mut Vec<String>) {
    let Some(activity) = app.session.codex_panel().activity_for(id) else {
        return;
    };
    let view = super::child_activity_view::ActivityView {
        error: activity.error(),
        known: !activity.reading() && activity.error().is_none(),
        reading: activity.reading(),
        count: activity.items().len(),
        older: activity.more(),
        tail: activity.tail(),
    };
    let records = activity.items().iter().rev().map(|entry| {
        let item = &entry["item"];
        let content = carlos_core::session::task_activity_text::display_item(item, 32 * 1024);
        super::child_activity_view::ActivityRecord {
            turn: entry["turnId"].as_str().unwrap_or("unavailable"),
            item: item["id"].as_str().unwrap_or("unavailable"),
            kind: item["type"].as_str().unwrap_or("unavailable"),
            text: std::borrow::Cow::Owned(content.text),
            complete: content.complete,
        }
    });
    lines.extend(super::child_activity_view::lines(view, records));
}

fn process_row(
    app: &AppState,
    thread: &str,
    id: &str,
    item: Option<&str>,
    lines: &mut Vec<String>,
) {
    let row = app
        .session
        .codex_panel()
        .processes()
        .iter()
        .find(|row| {
            row.thread() == thread
                && row.data()["processId"] == id
                && row.data()["itemId"].as_str() == item
        })
        .expect("selected process");
    lines.extend([
        format!("Process: {id}"),
        format!("Thread: {thread}"),
        format!("Item: {}", text(&row.data()["itemId"])),
        format!("Command: {}", text(&row.data()["command"])),
        format!("Directory: {}", text(&row.data()["cwd"])),
        format!(
            "OS PID: {} · CPU: {} · RSS KiB: {}",
            text(&row.data()["osPid"]),
            text(&row.data()["cpuPercent"]),
            text(&row.data()["rssKb"])
        ),
    ]);
    lines.push(
        "Present in the native background-process list; x terminates only this process.".into(),
    );
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
