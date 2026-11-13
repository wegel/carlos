//! Explain native goal state, budget semantics and pending work without inventing execution progress.

use carlos_core::session::codex::goal_state::GoalKnowledge;

use super::codex_goal_state::GoalConfirm;
use super::state::AppState;
use super::TerminalSize;

/// Put errors and the active editable field before long objectives on a narrow screen.
pub(super) fn body(app: &AppState, size: TerminalSize) -> Vec<String> {
    let state = &app.view.codex_goals;
    let facts = app.session.codex_goals();
    let mut lines = Vec::new();
    if state.editing && (size.width < 22 || size.height < 12) {
        lines.push("Resize to edit; the draft is kept".into());
    }
    if let Some(error) = facts.read_error() {
        lines.push(error.to_string());
    }
    if let Some(error) = super::codex_goals::error(app) {
        lines.push(error.to_string());
    }
    if facts.pending() {
        lines.push("Goal change pending; the draft is locked until the reply".into());
    }
    if let Some(confirm) = &state.confirm {
        match confirm {
            GoalConfirm::Activate(params) => {
                lines.push("Start/resume this Codex goal? Codex may start model work immediately. y confirms; n keeps the draft.".into());
                lines.push(format!("Objective: {}",params["objective"].as_str().or(facts.goal()["objective"].as_str()).unwrap_or("unavailable")));
            }
            GoalConfirm::Clear => lines.push("Clear the saved goal and its counters? A current turn may continue. y clears; n keeps the goal.".into()),
        }
        return safe(lines);
    }
    if state.editing {
        editor_lines(app, &mut lines);
    } else {
        native_lines(app, &mut lines);
    }

    safe(lines)
}

fn safe(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .map(|line| carlos_core::session::text::safe_text(&line))
        .collect()
}

fn editor_lines(app: &AppState, lines: &mut Vec<String>) {
    let state = &app.view.codex_goals;
    let facts = app.session.codex_goals();
    let draft = state.editor.as_ref().expect("goal draft");
    lines.push(format!(
        "Field {}/3: {}",
        draft.field + 1,
        ["Objective", "Token budget", "Status"][draft.field]
    ));
    lines.push(format!(
        "Status: {}",
        draft
            .status
            .as_deref()
            .or(facts.goal()["status"].as_str())
            .unwrap_or("paused")
    ));
    if draft.status.is_none() {
        lines
            .push("Save keeps the observed status explicitly; it never defaults to active.".into());
    }
    lines.push("Blank budget keeps the existing budget; it does not remove it.".into());
    if draft.field == 2 {
        lines.push("Left chooses paused; Right chooses active. Active can start work now.".into());
    }
    lines.push(format!(
        "Draft objective: {}",
        draft.objective.lines().join("\n")
    ));
}

fn native_lines(app: &AppState, lines: &mut Vec<String>) {
    let facts = app.session.codex_goals();
    let known = match facts.knowledge() {
        GoalKnowledge::Unknown => "Unknown",
        GoalKnowledge::Known => "Known",
        GoalKnowledge::Unavailable => "Unavailable/disabled",
    };
    lines.push(format!(
        "Goal state: {known}{}",
        if facts.reading() { " (reading)" } else { "" }
    ));
    lines.push(format!("Thread: {}", app.session.thread_id()));
    lines.push(format!(
        "Ralph: {}",
        if app.ralph_enabled() {
            "on"
        } else if app.session.ralph_runtime().toggle_pending() {
            "toggle pending"
        } else {
            "off"
        }
    ));
    goal_values(facts.goal(), lines);
    if !facts.notice().is_empty() {
        lines.push(facts.notice().to_string());
    }
    lines.push("Carlos never creates a goal from ordinary chat. Active goals and Ralph cannot drive the thread together.".into());
    lines.push("To remove a budget/reset counters: explicitly clear this goal, then create a new one. Edits keep counters.".into());
}

fn goal_values(goal: &serde_json::Value, lines: &mut Vec<String>) {
    if goal.is_null() {
        lines.push("No saved goal reported. e opens a paused goal draft.".into());
    } else {
        for (label, key) in [
            ("Status", "status"),
            ("Token budget", "tokenBudget"),
            ("Tokens used", "tokensUsed"),
            ("Time used (seconds)", "timeUsedSeconds"),
        ] {
            let value = &goal[key];
            let text = if value.is_null() {
                if key == "tokenBudget" {
                    "No reported budget".into()
                } else {
                    "unavailable".into()
                }
            } else {
                value
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| value.to_string())
            };
            lines.push(format!("{label}: {text}"));
        }
        lines.push(format!(
            "Objective: {}",
            goal["objective"].as_str().unwrap_or("unavailable")
        ));
    }
}
