//! Prepare addressed native run reads and export their checked results without any client focus.

use carlos_proto::commands::RpcError;
use carlos_proto::ids::{TaskId, TaskReadId};
use carlos_proto::tasks::TaskRunSnapshot;

use super::codex::panel_runs;
use super::state::SessionState;
use super::task_rows::TaskRef;
use super::task_windows::WindowContext;

impl SessionState {
    #[cfg(test)]
    pub(super) fn prepare_task_run(
        &mut self,
        task: &TaskId,
        read: Option<&TaskReadId>,
        refresh: bool,
    ) -> Result<(), RpcError> {
        self.prepare_task_run_in(None, task, read, refresh)
    }

    pub(super) fn prepare_task_run_in(
        &mut self,
        context: Option<&WindowContext>,
        task: &TaskId,
        read: Option<&TaskReadId>,
        refresh: bool,
    ) -> Result<(), RpcError> {
        let rows = self.task_rows_in(context)?;
        let row = rows
            .iter()
            .find(|row| row.id(self) == *task)
            .ok_or_else(|| failure("retired or foreign task"))?;
        if let Some(id) = read {
            if refresh {
                return Err(failure("refresh cannot continue an old task run read"));
            }
            let key = if matches!(row, TaskRef::WindowProcess { .. }) {
                super::codex::panel_process_runs::key(task)
            } else {
                row.native_id().to_string()
            };
            if !matches!(row, TaskRef::Agent { .. } | TaskRef::WindowProcess { .. })
                || !self.codex_panel.runs.get(&key).is_some_and(|read| {
                    &read.id == id
                        && read.matches_scope(self)
                        && panel_runs::same_window(read.window.as_ref(), context)
                })
            {
                return Err(failure(
                    "task run read was retired or superseded; original intent was not retargeted",
                ));
            }
            return Ok(());
        }
        if let (Some(context), TaskRef::WindowProcess { .. }) = (context, row) {
            super::codex::panel_process_runs::begin(self, context, task)
                .map_err(|error| failure(&error))?;
            return Ok(());
        }
        if let TaskRef::Agent { .. } = row {
            let thread = row.native_id().to_string();
            if !self.events.backend_closed {
                panel_runs::begin_in(self, &thread, refresh, context.cloned())
                    .map_err(|message| failure(&message))?;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn task_run_snapshot(&self, task: &TaskId) -> Result<TaskRunSnapshot, RpcError> {
        self.task_run_snapshot_in(None, task)
    }

    pub(super) fn task_run_snapshot_in(
        &self,
        context: Option<&WindowContext>,
        task: &TaskId,
    ) -> Result<TaskRunSnapshot, RpcError> {
        let rows = self.task_control_rows_in(context)?;
        let row = rows
            .iter()
            .find(|row| row.id(self) == *task)
            .ok_or_else(|| failure("retired or foreign task"))?;
        let key = if matches!(row, TaskRef::WindowProcess { .. }) {
            super::codex::panel_process_runs::key(task)
        } else {
            row.native_id().to_string()
        };
        let read = matches!(row, TaskRef::Agent { .. } | TaskRef::WindowProcess { .. })
            .then(|| self.codex_panel.runs.get(&key))
            .flatten()
            .filter(|read| {
                read.matches_scope(self) && panel_runs::same_window(read.window.as_ref(), context)
            });
        let state = match row {
            TaskRef::Agent { .. } => row.agent_run().map(|(_, status)| status.to_string()),
            _ => Some(row.export(self).state),
        };
        let process_verified =
            read.is_some_and(|read| super::codex::panel_process_runs::verified(read, self));
        let run = row
            .run(self)
            .filter(|_| !matches!(row, TaskRef::WindowProcess { .. }) || process_verified);
        let accepted = matches!(
            row,
            TaskRef::Agent {
                accepted_run: Some(_),
                ..
            }
        );
        let known = run.is_some() || accepted;
        Ok(TaskRunSnapshot {
            session: self.session_id().clone(),
            task: task.clone(),
            read: read.map(|read| read.id.clone()),
            run,
            known,
            native_state: state,
            reading: read.is_some_and(|read| read.request.is_some()),
            closed: self.events.backend_closed,
            error: self.task_run_error(row, read, accepted),
        })
    }

    fn task_run_error(
        &self,
        row: &TaskRef<'_>,
        read: Option<&super::codex::panel_runs::RunRead>,
        accepted: bool,
    ) -> Option<String> {
        if let TaskRef::WindowProcess { error, .. } = row {
            read.and_then(|read| {
                read.error.clone().or_else(|| {
                    (!super::codex::panel_process_runs::current(read, self))
                        .then(|| "Original process changed while reading; no stop sent".into())
                })
            })
            .or_else(|| error.map(str::to_string))
        } else if let TaskRef::Process(row) = row {
            self.process_read_error(&row.thread).map(str::to_string)
        } else if accepted {
            None
        } else {
            read.and_then(|read| read.read_error(self, row.native_id()))
        }
    }
}

fn failure(message: &str) -> RpcError {
    RpcError {
        code: -32012,
        message: message.into(),
    }
}
