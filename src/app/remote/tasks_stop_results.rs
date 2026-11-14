//! Retain only this terminal's task-stop progress without clearing or replacing any text draft.

use std::collections::VecDeque;

use carlos_client::Client;
use carlos_proto::ids::{TaskId, TaskRun};
use carlos_proto::submissions::{ClientOperation, OperationState};
use carlos_proto::tasks::Task;

use super::Device;

pub(super) struct OwnedStop {
    pub(super) task: TaskId,
    pub(super) run: TaskRun,
}
struct Notice {
    task: TaskId,
    run: TaskRun,
    text: String,
    rejected: bool,
}

#[derive(Default)]
pub(super) struct StopResults {
    pub(super) owned: Vec<(ClientOperation, OwnedStop)>,
    notices: VecDeque<Notice>,
}

impl StopResults {
    pub(super) fn pending(&self, client: &Client, task: &TaskId) -> bool {
        self.owned.iter().any(|(operation, owned)| {
            &owned.task == task
                && client.operations().get(operation).is_some_and(|record| {
                    matches!(
                        record.state,
                        OperationState::Pending | OperationState::Queued
                    )
                })
        })
    }

    fn remember(&mut self, owned: OwnedStop, text: String, rejected: bool) {
        self.notices
            .retain(|old| old.task != owned.task || old.run != owned.run);
        self.notices.push_back(Notice {
            task: owned.task,
            run: owned.run,
            text,
            rejected,
        });
        while self.notices.len() > 128 {
            self.notices.pop_front();
        }
    }
}

impl Device {
    pub(super) fn task_operations_changed(&self, operations: &[ClientOperation]) -> bool {
        operations.iter().any(|operation| {
            self.tasks
                .stops
                .results
                .owned
                .iter()
                .any(|(owned, _)| owned == operation)
        })
    }

    /// Keep final stop notices private and release settled operation slots without touching the composer.
    pub(super) fn apply_task_operation(&mut self, operation: &ClientOperation) -> bool {
        let Some(index) = self
            .tasks
            .stops
            .results
            .owned
            .iter()
            .position(|(id, _)| id == operation)
        else {
            return false;
        };
        let Some(record) = self.client.operations().get(operation) else {
            return true;
        };
        let (text, rejected) = match &record.state {
            OperationState::Accepted { .. } => (
                "Stop requested; waiting for native task event".into(),
                false,
            ),
            OperationState::Rejected { message, .. } => {
                (format!("Stop failed: {}", display_message(message)), true)
            }
            OperationState::Pending | OperationState::Queued | OperationState::Uncertain { .. } => {
                return true
            }
        };
        let (_, owned) = self.tasks.stops.results.owned.remove(index);
        self.tasks.stops.results.remember(owned, text, rejected);
        self.client.forget_operation(operation);
        true
    }

    pub(super) fn task_stop_notice(&self, task: &Task) -> Option<String> {
        let ended = matches!(
            task.state.as_str(),
            "completed" | "failed" | "stopped" | "interrupted"
        );
        self.task_stop_notice_for(&task.id, task.run.as_ref(), ended)
    }

    pub(super) fn missing_task_stop_notice(&self) -> Option<String> {
        self.task_stop_notice_for(self.tasks.view.selected.as_ref()?, None, true)
    }

    fn task_stop_notice_for(
        &self,
        task: &TaskId,
        run: Option<&TaskRun>,
        ended: bool,
    ) -> Option<String> {
        if self.tasks.stops.waiting_for(task) {
            return Some("Reading current native run before stop…".into());
        }
        let results = &self.tasks.stops.results;
        for (operation, owned) in results
            .owned
            .iter()
            .rev()
            .filter(|(_, owned)| &owned.task == task)
        {
            let Some(record) = self.client.operations().get(operation) else {
                continue;
            };
            if let Some(notice) = live_notice(
                &record.state,
                Some(&owned.run) != run,
                self.client.current_session_is_ready(&self.session),
            ) {
                return Some(notice);
            }
        }
        results
            .notices
            .iter()
            .rev()
            .filter(|notice| &notice.task == task)
            .find_map(|notice| notice.for_run(run, ended))
    }
}

impl Notice {
    fn for_run(&self, run: Option<&TaskRun>, ended: bool) -> Option<String> {
        let same = Some(&self.run) == run;
        if !self.rejected && (ended || !same) {
            return None;
        }
        Some(if same {
            self.text.clone()
        } else if run.is_some() {
            format!("Prior-run: {}", self.text)
        } else {
            format!("Original run: {}", self.text)
        })
    }
}

fn live_notice(state: &OperationState, prior: bool, ready: bool) -> Option<String> {
    let prefix = if prior { "Prior-run: " } else { "" };
    let text = match state {
        OperationState::Pending | OperationState::Queued => {
            if !ready {
                "Stop pending; waiting for accepted connection. No automatic retry".into()
            } else if prior {
                "Stop pending; waiting for original native acknowledgement".into()
            } else {
                "Sending stop request".into()
            }
        }
        OperationState::Uncertain { message, .. } => format!(
            "stop uncertain: {}. No automatic retry",
            display_message(message)
        ),
        _ => return None,
    };
    Some(format!("{prefix}{text}"))
}

fn display_message(message: &str) -> String {
    let mut text: String = message.chars().take(2048).collect();
    if message.chars().nth(2048).is_some() {
        text.push_str(" [display limited]");
    }
    text
}

#[cfg(test)]
#[path = "tasks_stop_results_tests.rs"]
mod tests;
