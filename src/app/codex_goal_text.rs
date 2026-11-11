//! Explain native goal state, budget semantics and pending work without inventing execution progress.

use super::codex_goal_state::{GoalConfirm, GoalKnowledge};
use super::state::AppState;
use super::TerminalSize;

/// Put errors and the active editable field before long objectives on a narrow screen.
pub(super) fn body(app: &AppState, size: TerminalSize) -> Vec<String> {
    let state = &app.codex.goals;
    let mut lines = Vec::new();
    if state.editing && (size.width < 22 || size.height < 12) {
        lines.push("Resize to edit; the draft is kept".into());
    }
    if let Some(error) = &state.read_error {
        lines.push(error.clone());
    }
    if let Some(error) = &state.error {
        lines.push(error.clone());
    }
    if state.mutation.is_some() {
        lines.push("Goal change pending; the draft is locked until the reply".into());
    }
    if let Some(confirm) = &state.confirm {
        match confirm {
            GoalConfirm::Activate(params) => {
                lines.push("Start/resume this Codex goal? Codex may start model work immediately. y confirms; n keeps the draft.".into());
                lines.push(format!("Objective: {}",params["objective"].as_str().or(state.goal["objective"].as_str()).unwrap_or("unavailable")));
            }
            GoalConfirm::Clear => lines.push("Clear the saved goal and its counters? A current turn may continue. y clears; n keeps the goal.".into()),
        }
        return safe(lines);
    }
    if state.editing {
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
                .or(state.goal["status"].as_str())
                .unwrap_or("paused")
        ));
        if draft.status.is_none() {
            lines.push(
                "Save keeps the observed status explicitly; it never defaults to active.".into(),
            );
        }
        lines.push("Blank budget keeps the existing budget; it does not remove it.".into());
        if draft.field == 2 {
            lines.push(
                "Left chooses paused; Right chooses active. Active can start work now.".into(),
            );
        }
        lines.push(format!(
            "Draft objective: {}",
            draft.objective.lines().join("\n")
        ));
    } else {
        let known = match state.knowledge {
            GoalKnowledge::Unknown => "Unknown",
            GoalKnowledge::Known => "Known",
            GoalKnowledge::Unavailable => "Unavailable/disabled",
        };
        lines.push(format!(
            "Goal state: {known}{}",
            if state.read.is_some() {
                " (reading)"
            } else {
                ""
            }
        ));
        lines.push(format!("Thread: {}", app.thread_id));
        lines.push(format!(
            "Ralph: {}",
            if app.ralph_enabled() {
                "on"
            } else if app.ralph_runtime.toggle_pending() {
                "toggle pending"
            } else {
                "off"
            }
        ));
        if state.goal.is_null() {
            lines.push("No saved goal reported. e opens a paused goal draft.".into());
        } else {
            for (label, key) in [
                ("Status", "status"),
                ("Token budget", "tokenBudget"),
                ("Tokens used", "tokensUsed"),
                ("Time used (seconds)", "timeUsedSeconds"),
            ] {
                let value = &state.goal[key];
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
                state.goal["objective"].as_str().unwrap_or("unavailable")
            ));
        }
        if !state.notice.is_empty() {
            lines.push(state.notice.clone());
        }
        lines.push("Carlos never creates a goal from ordinary chat. Active goals and Ralph cannot drive the thread together.".into());
        lines.push("To remove a budget/reset counters: explicitly clear this goal, then create a new one. Edits keep counters.".into());
    }
    safe(lines)
}

fn safe(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .map(|line| super::claude_question_state::safe_text(&line))
        .collect()
}
