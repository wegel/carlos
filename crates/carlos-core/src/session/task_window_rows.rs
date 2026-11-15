//! Borrow a captured native window through the same task rows used by the latest-list widgets.

use carlos_proto::commands::RpcError;
use carlos_proto::ids::TaskRevision;

use super::state::SessionState;
use super::task_rows::TaskRef;
use super::task_windows::WindowContext;

impl SessionState {
    pub(in crate::session) fn window_process_verified(
        &self,
        context: &WindowContext,
        task: &carlos_proto::ids::TaskId,
    ) -> bool {
        self.codex_panel
            .runs
            .get(&super::codex::panel_process_runs::key(task))
            .is_some_and(|read| {
                super::codex::panel_runs::same_window(read.window.as_ref(), Some(context))
                    && super::codex::panel_process_runs::verified(read, self)
            })
    }

    pub(in crate::session) fn window_process_owners(&self) -> std::collections::HashSet<String> {
        self.task_windows
            .slots
            .values()
            .filter(|slot| slot.reader.load(std::sync::atomic::Ordering::Acquire))
            .filter_map(|slot| {
                if let super::task_window_state::Contents::Ready(data) = &slot.contents {
                    Some(data)
                } else {
                    None
                }
            })
            .flat_map(|data| data.processes.iter().map(|row| row.thread.clone()))
            .collect()
    }
    pub(super) fn task_control_rows_in(
        &self,
        context: Option<&WindowContext>,
    ) -> Result<Vec<TaskRef<'_>>, RpcError> {
        let mut rows = self.task_rows_in(context)?;
        for row in &mut rows {
            if let TaskRef::Agent {
                data, accepted_run, ..
            } = row
            {
                *accepted_run = data["id"]
                    .as_str()
                    .and_then(|id| super::codex::panel_runs::ready_in(self, id, context));
            }
        }
        Ok(rows)
    }
    pub(super) fn task_rows_in(
        &self,
        context: Option<&WindowContext>,
    ) -> Result<Vec<TaskRef<'_>>, RpcError> {
        let Some(context) = context else {
            return Ok(self.task_rows());
        };
        let data = self.task_window_capture(context)?;
        let owns = |id: &str| {
            id == self.thread_id()
                || data
                    .owners
                    .contains(self.thread_id(), self.codex_scope().epoch(), id)
        };
        Ok(data
            .agents
            .iter()
            .filter(|agent| agent["id"].as_str().is_some_and(owns))
            .map(|agent| TaskRef::Agent {
                data: agent,
                accepted_run: None,
                report: agent["id"].as_str().and_then(|id| data.reports.get(id)),
            })
            .chain(
                data.processes
                    .iter()
                    .filter(|row| owns(&row.thread))
                    .map(|row| TaskRef::WindowProcess {
                        row,
                        error: data
                            .process_scopes
                            .get(&row.thread)
                            .and_then(|error| error.as_deref()),
                    }),
            )
            .collect())
    }

    pub(super) fn task_revision_in(
        &self,
        context: Option<&WindowContext>,
        rows: &[TaskRef<'_>],
    ) -> Result<TaskRevision, RpcError> {
        match context {
            Some(context) => Ok(self.task_window_capture(context)?.revision.clone()),
            None => Ok(self.task_revision(rows)),
        }
    }
}
