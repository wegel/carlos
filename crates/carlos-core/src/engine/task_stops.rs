//! Keep native task stop outcomes private while their existing asynchronous requests settle.

use std::time::{Duration, Instant};

use carlos_proto::ids::{TaskId, TaskRun};
use carlos_proto::submissions::{ClientOperation, OperationState};

use super::owner::Owner;
use super::target::Target;
use super::workers::Completion;
use crate::session::task_stop_reply::TaskStopReply;
use crate::session::task_stop_target::StopTarget;
use crate::session::task_stop_work::TaskStopWork;

pub(super) struct PendingTaskStop {
    work: TaskStopWork,
    target: Target,
    started: Instant,
}

impl Owner {
    pub(super) fn stop_task(
        &mut self,
        window: Option<crate::session::task_windows::WindowContext>,
        operation: ClientOperation,
        task: TaskId,
        expected: TaskRun,
    ) {
        if self.native.closed {
            self.results.update(
                &operation,
                super::dispatch::rejected("native backend is closed; task stop was not sent"),
            );
            self.notice();
            return;
        }
        let commands = match self.client.capture_commands() {
            Ok(commands) => commands,
            Err(error) => {
                self.results
                    .update(&operation, super::dispatch::rejected(error.to_string()));
                self.notice();
                return;
            }
        };
        let work =
            match self
                .session
                .prepare_task_stop_in(window, operation.clone(), task, expected)
            {
                Ok(work) => work,
                Err(error) => {
                    self.results
                        .update(&operation, super::dispatch::rejected(error.message));
                    self.notice();
                    return;
                }
            };
        let target = Target::capture(&self.session, self.client.kind());
        self.task_stops.insert(
            operation.clone(),
            PendingTaskStop {
                work: work.clone(),
                target,
                started: Instant::now(),
            },
        );
        self.workers.controls_busy += 1;
        self.workers
            .controls
            .submit(move || Completion::TaskStopDispatch {
                operation,
                result: work.dispatch(&commands).map_err(|error| error.to_string()),
            });
        self.notice();
    }

    pub(super) fn finish_task_stop_dispatch(
        &mut self,
        operation: ClientOperation,
        result: Result<(), String>,
    ) {
        self.workers.controls_busy -= 1;
        let Err(error) = result else {
            return;
        };
        let Some(pending) = self.task_stops.remove(&operation) else {
            // A native acknowledgement may beat the writer's local completion.
            return;
        };
        let error: String = error.chars().take(512).collect();
        let message = crate::session::text::safe_text(&format!(
            "Native task stop could not be confirmed: {error}"
        ));
        self.fail_task_stop(&pending.work, &pending.target, &message);
        self.session.task_stops.finish(&operation);
        self.results.update(&operation, uncertain(&message));
        self.notice();
    }

    pub(super) fn finish_task_stop(&mut self, reply: TaskStopReply) {
        let Some(pending) = self.task_stops.remove(&reply.operation) else {
            return;
        };
        let matching = pending.work.claim.task == reply.task
            && pending.work.claim.run == reply.run
            && pending.work.claim.target.kind() == reply.kind
            && pending.target.matches(&self.session, self.client.kind());
        let state = if matching {
            reply.state
        } else {
            uncertain("task stop acknowledgement belongs to a retired or mismatched target")
        };
        if matches!(state, OperationState::Accepted { .. }) {
            pending.work.acknowledged(&mut self.session);
        } else if let OperationState::Rejected { message, .. }
        | OperationState::Uncertain { message, .. } = &state
        {
            self.fail_task_stop(&pending.work, &pending.target, message);
        }
        self.session.task_stops.finish(&reply.operation);
        self.results.update(&reply.operation, state);
        self.notice();
    }

    pub(super) fn retire_task_stops(&mut self) {
        self.session.task_stops.refresh_processes(&self.session);
        let retired: Vec<_> = self
            .task_stops
            .iter()
            .filter(|(_, pending)| {
                self.native.closed
                    || !pending.target.matches(&self.session, self.client.kind())
                    || pending.started.elapsed() >= Duration::from_secs(15)
            })
            .map(|(operation, _)| operation.clone())
            .collect();
        for operation in retired {
            let pending = self
                .task_stops
                .remove(&operation)
                .expect("retired task stop");
            let message = "native task stop was not confirmed before its root, connection or deadline retired";
            self.fail_task_stop(&pending.work, &pending.target, message);
            self.session.task_stops.finish(&operation);
            self.results.update(&operation, uncertain(message));
            self.notice();
        }
    }

    fn fail_task_stop(&mut self, work: &TaskStopWork, target: &Target, message: &str) {
        if target.matches(&self.session, self.client.kind()) {
            if let StopTarget::Claude { task, run } = &work.claim.target {
                self.session
                    .receive_claude_task_stop(task, Some(run), Some(message.into()));
            }
        }
    }
}

fn uncertain(message: &str) -> OperationState {
    OperationState::Uncertain {
        message: message.into(),
        recovery: None,
    }
}

#[cfg(test)]
#[path = "task_stop_tests.rs"]
mod tests;
