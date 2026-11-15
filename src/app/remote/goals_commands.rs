//! Submit revision-checked goal changes and settle only this terminal's private results.

use anyhow::{bail, Result};
use carlos_proto::commands::Command;
use carlos_proto::goals::{GoalChange, GoalKnowledge, GoalStatus};
use carlos_proto::ids::Sequence;
use carlos_proto::submissions::{ClientOperation, OperationState};
use serde_json::json;

use super::goals::GoalAction;
use super::transport::Transport;
use super::Device;
use crate::app::codex_goal_state::GoalConfirm;

impl Device {
    /// Validate this draft and freeze its explicit status before submitting it.
    pub(super) fn save_goal(&mut self, link: Option<&mut Transport>) -> Result<()> {
        let Some(draft) = self.view.codex_goals.editor.as_ref() else {
            return Ok(());
        };
        let params = draft.params("").map_err(anyhow::Error::msg)?;
        let status = params["status"]
            .as_str()
            .or_else(|| {
                self.goal_facts()?
                    .goal
                    .as_ref()
                    .map(|goal| goal.status.as_str())
            })
            .unwrap_or("paused");
        let status = parse_status(status)?;
        let change = GoalChange {
            objective: params["objective"].as_str().map(str::to_string),
            token_budget: params["tokenBudget"].as_u64().map(Sequence::new),
            status,
        };
        self.prepare_goal(link, change)
    }

    /// Change only the current goal status while retaining any unsent draft.
    pub(super) fn goal_status(&mut self, link: Option<&mut Transport>, active: bool) -> Result<()> {
        if self
            .goal_facts()
            .and_then(|facts| facts.goal.as_ref())
            .is_none()
        {
            bail!("No goal exists; e creates a paused draft");
        }
        self.prepare_goal(
            link,
            GoalChange {
                objective: None,
                token_budget: None,
                status: if active {
                    GoalStatus::Active
                } else {
                    GoalStatus::Paused
                },
            },
        )
    }

    fn prepare_goal(&mut self, link: Option<&mut Transport>, change: GoalChange) -> Result<()> {
        self.can_change_goal(change.status == GoalStatus::Active)?;
        let expected = self
            .goal_facts()
            .expect("checked goal facts")
            .revision
            .clone();
        if change.status == GoalStatus::Active {
            self.view.codex_goals.scroll = 0;
            self.view.codex_goals.confirm =
                Some(GoalConfirm::Activate(json!({"objective":change.objective})));
            self.goals.confirm = Some(GoalAction {
                expected,
                change: Some(change),
            });
        } else {
            self.send_goal(
                link,
                GoalAction {
                    expected,
                    change: Some(change),
                },
            )?;
        }
        Ok(())
    }

    /// Ask for explicit consent before clearing the saved goal and its counters.
    pub(super) fn prepare_goal_clear(&mut self) -> Result<()> {
        self.can_change_goal(false)?;
        self.goals.confirm = Some(GoalAction {
            expected: self
                .goal_facts()
                .expect("checked goal facts")
                .revision
                .clone(),
            change: None,
        });
        self.view.codex_goals.scroll = 0;
        self.view.codex_goals.confirm = Some(GoalConfirm::Clear);
        Ok(())
    }

    /// Send only the revision that the user explicitly chose to confirm.
    pub(super) fn confirm_goal(&mut self, link: Option<&mut Transport>) -> Result<()> {
        let Some(action) = self.goals.confirm.take() else {
            return Ok(());
        };
        self.view.codex_goals.confirm = None;
        self.can_change_goal(
            action
                .change
                .as_ref()
                .is_some_and(|change| change.status == GoalStatus::Active),
        )?;
        self.send_goal(link, action)
    }

    fn can_change_goal(&self, active: bool) -> Result<()> {
        if self.goal_pending() {
            bail!("A pending Codex goal change must finish first");
        }
        let facts = self.goal_facts().ok_or_else(|| {
            anyhow::anyhow!("Read the current Codex goal before editing; r refreshes")
        })?;
        if facts.knowledge != GoalKnowledge::Known {
            bail!("Read the current Codex goal before editing; r refreshes");
        }
        if active
            && (self.goals.ralph.is_some()
                || self
                    .client
                    .session(&self.session)
                    .is_some_and(|replica| replica.info().ralph))
        {
            bail!("Disable Ralph before activating a Codex goal");
        }
        Ok(())
    }

    fn send_goal(&mut self, link: Option<&mut Transport>, action: GoalAction) -> Result<()> {
        let link = link.ok_or_else(|| anyhow::anyhow!("Disconnected; F5 reconnects"))?;
        let session = self.session.clone();
        let clear_editor = action
            .change
            .as_ref()
            .is_some_and(|change| change.objective.is_some());
        let mut submitted = None;
        let outgoing = self.client.submit(link.connection, |operation| {
            submitted = Some(operation.clone());
            match action.change {
                Some(change) => Command::GoalSet {
                    session,
                    operation,
                    expected: action.expected,
                    change,
                },
                None => Command::GoalClear {
                    session,
                    operation,
                    expected: action.expected,
                },
            }
        })?;
        self.goals.operation = submitted;
        self.goals.clear_editor = clear_editor;
        self.view.codex_goals.error = None;
        link.send(outgoing)
    }

    /// Settle only this terminal's goal or Ralph command without touching other drafts.
    pub(super) fn apply_goal_operation(&mut self, id: &ClientOperation) -> bool {
        let is_goal = self.goals.operation.as_ref() == Some(id);
        let is_ralph = self.goals.ralph.as_ref() == Some(id);
        if !is_goal && !is_ralph {
            return false;
        }
        let Some(record) = self.client.operations().get(id) else {
            return true;
        };
        match &record.state {
            OperationState::Accepted { .. } => {
                if is_goal {
                    self.view.codex_goals.error = None;
                }
                if is_goal && self.goals.clear_editor {
                    self.view.codex_goals.editor = None;
                    self.view.codex_goals.editing = false;
                }
            }
            OperationState::Rejected { message, .. } => {
                let message = message.clone();
                self.goal_operation_failed(&message, is_ralph);
            }
            OperationState::Uncertain { message, .. } => {
                let message = message.clone();
                self.goal_operation_failed(&message, is_ralph);
                if is_goal {
                    self.goals.clear_editor = false;
                }
                if is_ralph {
                    self.goals.ralph = None;
                }
                return true;
            }
            OperationState::Pending | OperationState::Queued => return true,
        }
        if is_goal {
            self.goals.operation = None;
        }
        if is_ralph {
            self.goals.ralph = None;
        }
        self.client.forget_operation(id);
        true
    }

    fn goal_operation_failed(&mut self, message: &str, is_ralph: bool) {
        self.view.codex_goals.error = Some(message.to_owned());
        if is_ralph {
            self.view.status_hint = Some(format!("ralph: {message}"));
        }
    }
}

fn parse_status(status: &str) -> Result<GoalStatus> {
    Ok(match status {
        "active" => GoalStatus::Active,
        "paused" => GoalStatus::Paused,
        "blocked" => GoalStatus::Blocked,
        "usageLimited" => GoalStatus::UsageLimited,
        "budgetLimited" => GoalStatus::BudgetLimited,
        "complete" => GoalStatus::Complete,
        _ => bail!("Unknown goal status; explicitly choose paused or active before saving"),
    })
}
