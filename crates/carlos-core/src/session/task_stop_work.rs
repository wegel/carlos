//! Prepare one task stop and carry its core permission to the existing native writer.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::{ensure, Result};
use carlos_proto::commands::RpcError;
use carlos_proto::ids::{TaskId, TaskRun};
use carlos_proto::submissions::ClientOperation;
use serde_json::{json, Value};

use super::codex::panel_pages::valid_id;
use super::state::SessionState;
use super::task_rows::TaskRef;
use super::task_stop_claims::StopClaim;
use super::task_stop_target::{process_identity, StopTarget};
use crate::backend::BackendKind;
use crate::backend_commands::CommandHandle;
use crate::dispatch_guard::DispatchGuard;

/// This command owns no session borrow and enqueues only an already checked native target.
#[derive(Clone)]
pub(in crate::session) struct TaskStopWork {
    pub claim: Arc<StopClaim>,
    context: Value,
}

impl TaskStopWork {
    pub fn dispatch(&self, commands: &CommandHandle) -> Result<()> {
        ensure!(
            commands.kind() == self.claim.target.backend(),
            "task stop backend changed before dispatch"
        );
        let claim = self.claim.clone();
        let commands = commands.guarded(DispatchGuard::new(move || {
            claim.live.load(Ordering::SeqCst)
        }))?;
        let (method, mut params) = self.claim.target.request();
        if commands.kind() == BackendKind::Claude {
            params["taskStopContext"] = self.context.clone();
            commands.request_native_control(method, params)
        } else {
            commands.request_control_with_context(method, params, self.context.clone())
        }
    }

    pub fn acknowledged(&self, session: &mut SessionState) {
        self.claim.target.acknowledged(session);
    }
}

impl SessionState {
    pub(in crate::session) fn prepare_task_stop(
        &mut self,
        operation: ClientOperation,
        task: TaskId,
        expected: TaskRun,
    ) -> Result<TaskStopWork, RpcError> {
        let target = self.task_stop_target(&task, &expected)?;
        let claim =
            self.task_stops
                .begin(operation.clone(), task.clone(), expected.clone(), target)?;
        if let StopTarget::Claude { task, run } = &claim.target {
            match self.claude.tasks.stops.begin(task, Some(run)) {
                Ok(true) => {}
                result => {
                    self.task_stops.finish(&operation);
                    return Err(failure(result.err().unwrap_or(
                        "a stop for this exact native run is already requested",
                    )));
                }
            }
        }
        Ok(TaskStopWork {
            claim,
            context: json!({"operation":"taskStop","clientOperation":operation,
            "task":task,"expectedRun":expected,"root":self.thread_id(),"epoch":self.codex_scope().epoch(),"timeoutMs":15000}),
        })
    }

    fn task_stop_target(&self, task: &TaskId, expected: &TaskRun) -> Result<StopTarget, RpcError> {
        if self.events.backend_closed {
            return Err(failure(
                "the native backend is closed; task stop was not sent",
            ));
        }
        let rows = self.task_rows();
        let row = rows
            .iter()
            .find(|row| row.id(self) == *task)
            .ok_or_else(|| failure("retired or foreign task"))?;
        if row.run(self).as_ref() != Some(expected) {
            return Err(failure(
                "the task run changed or is unknown; task stop was not sent",
            ));
        }
        match row {
            TaskRef::Agent { .. } => {
                let (turn, status) = row
                    .agent_run()
                    .ok_or_else(|| failure("no current native turn was verified"))?;
                if status != "inProgress" || row.native_id() == self.thread_id() {
                    return Err(failure("the selected child has no running native turn"));
                }
                Ok(StopTarget::Agent {
                    thread: row.native_id().into(),
                    turn: turn.into(),
                })
            }
            TaskRef::Process(process) => {
                let (item, pid) = process_identity(&process.data)
                    .ok_or_else(|| failure("native process identity is unsupported or unknown"))?;
                Ok(StopTarget::Process {
                    thread: process.thread.clone(),
                    process: valid_id(&process.data["processId"])
                        .map_err(|message| failure(&message))?
                        .into(),
                    item: item.map(str::to_string),
                    pid: pid.cloned(),
                })
            }
            TaskRef::Claude { id, data } => {
                if !self.claude.tasks.can_stop(id) {
                    return Err(failure(
                        "the selected native task has ended or is unavailable",
                    ));
                }
                if id.is_empty() || id.len() > 4096 {
                    return Err(failure("native task id is missing or oversized"));
                }
                Ok(StopTarget::Claude {
                    task: id.to_string(),
                    run: valid_id(&data["run_id"])
                        .map_err(|message| failure(&message))?
                        .into(),
                })
            }
        }
    }
}

fn failure(message: &str) -> RpcError {
    RpcError {
        code: -32012,
        message: message.into(),
    }
}
