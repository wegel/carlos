//! Prepare addressed native run reads and export their checked results without any client focus.

use carlos_proto::commands::RpcError;
use carlos_proto::ids::{TaskId, TaskReadId};
use carlos_proto::tasks::TaskRunSnapshot;

use super::codex::panel_runs;
use super::state::SessionState;
use super::task_rows::TaskRef;

impl SessionState {
    pub(super) fn prepare_task_run(
        &mut self,
        task: &TaskId,
        read: Option<&TaskReadId>,
        refresh: bool,
    ) -> Result<(), RpcError> {
        let rows = self.task_rows();
        let row = rows
            .iter()
            .find(|row| row.id(self) == *task)
            .ok_or_else(|| failure("retired or foreign task"))?;
        if let Some(id) = read {
            if refresh {
                return Err(failure("refresh cannot continue an old task run read"));
            }
            if !matches!(row, TaskRef::Agent { .. })
                || !self
                    .codex_panel
                    .runs
                    .get(row.native_id())
                    .is_some_and(|read| &read.id == id && read.matches_scope(self))
            {
                return Err(failure(
                    "task run read was retired or superseded; original intent was not retargeted",
                ));
            }
            return Ok(());
        }
        if let TaskRef::Agent { .. } = row {
            let thread = row.native_id().to_string();
            if !self.events.backend_closed {
                panel_runs::begin(self, &thread, refresh).map_err(|message| failure(&message))?;
            }
        }
        Ok(())
    }

    pub(super) fn task_run_snapshot(&self, task: &TaskId) -> Result<TaskRunSnapshot, RpcError> {
        let rows = self.task_rows();
        let row = rows
            .iter()
            .find(|row| row.id(self) == *task)
            .ok_or_else(|| failure("retired or foreign task"))?;
        let read = matches!(row, TaskRef::Agent { .. })
            .then(|| self.codex_panel.runs.get(row.native_id()))
            .flatten()
            .filter(|read| read.matches_scope(self));
        let state = match row {
            TaskRef::Agent { .. } => row.agent_run().map(|(_, status)| status.to_string()),
            _ => Some(row.export(self).state),
        };
        let run = row.run(self);
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
            error: if accepted {
                None
            } else {
                read.and_then(|read| read.read_error(self, row.native_id()))
            },
        })
    }
}

fn failure(message: &str) -> RpcError {
    RpcError {
        code: -32012,
        message: message.into(),
    }
}
