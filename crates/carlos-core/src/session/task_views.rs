//! Serve independently addressed task pages, metadata previews and exact scalar text chunks.

use carlos_proto::bytes::{ByteLength, ByteOffset};
use carlos_proto::commands::RpcError;
use carlos_proto::ids::{TaskId, TaskRevision};
use carlos_proto::tasks::{
    TaskDetail, TaskField, TaskPage, TaskProperty, TaskText, MAX_PAGE_TASKS,
};

use super::state::SessionState;
use super::task_fields::FIELDS;
use super::task_rows::{clipped, TaskRef};
use super::task_windows::WindowContext;

impl SessionState {
    pub(super) fn task_page(
        &self,
        expected: Option<&TaskRevision>,
        after: Option<&TaskId>,
        limit: usize,
    ) -> Result<TaskPage, RpcError> {
        self.task_page_in(None, expected, after, limit)
    }

    pub(super) fn task_page_in(
        &self,
        context: Option<&WindowContext>,
        expected: Option<&TaskRevision>,
        after: Option<&TaskId>,
        limit: usize,
    ) -> Result<TaskPage, RpcError> {
        let rows = self.task_rows_in(context)?;
        let revision = self.task_revision_in(context, &rows)?;
        require_revision(expected, &revision)?;
        if after.is_some() && expected.is_none() {
            return Err(failure("task page needs its expected revision"));
        }
        let start = after
            .map(|id| {
                rows.iter()
                    .position(|row| &row.id(self) == id)
                    .map(|index| index + 1)
                    .ok_or_else(|| failure("retired or foreign task page cursor"))
            })
            .transpose()?
            .unwrap_or(0);
        let end = start
            .saturating_add(limit.clamp(1, MAX_PAGE_TASKS))
            .min(rows.len());
        Ok(TaskPage {
            session: self.session_id().clone(),
            seq: self.events.seq,
            revision,
            known: context.is_some() || self.task_known(),
            reading: context.is_none() && self.codex_panel.snapshot.is_some(),
            closed: self.events.backend_closed,
            tasks: rows[start..end]
                .iter()
                .map(|row| row.export(self))
                .collect(),
            has_more: end < rows.len(),
            native_older_available: if let Some(context) = context {
                !self.task_window_capture(context)?.older.is_null()
            } else {
                !self.codex_panel.older.is_null()
            },
            read_error: if let Some(context) = context {
                self.task_window_capture(context)?
                    .read_error
                    .as_deref()
                    .map(|error| clipped(error, 2048).into())
            } else {
                self.codex_panel
                    .error
                    .as_deref()
                    .map(|error| clipped(error, 2048).into())
            },
        })
    }

    #[cfg(test)]
    pub(super) fn task_detail(
        &self,
        expected: &TaskRevision,
        id: &TaskId,
    ) -> Result<TaskDetail, RpcError> {
        self.task_detail_in(None, expected, id)
    }

    pub(super) fn task_detail_in(
        &self,
        context: Option<&WindowContext>,
        expected: &TaskRevision,
        id: &TaskId,
    ) -> Result<TaskDetail, RpcError> {
        let rows = self.task_rows_in(context)?;
        require_revision(Some(expected), &self.task_revision_in(context, &rows)?)?;
        let row = find(self, &rows, id)?;
        Ok(TaskDetail {
            session: self.session_id().clone(),
            revision: expected.clone(),
            task: row.export(self),
            properties: FIELDS
                .iter()
                .filter_map(|field| {
                    let value = row.field(self, *field)?;
                    let text = clipped(&value, 2048);
                    Some(TaskProperty {
                        field: *field,
                        text: text.into(),
                        bytes: ByteLength::new(value.len() as u64),
                        complete: text.len() == value.len(),
                    })
                })
                .collect(),
        })
    }

    #[cfg(test)]
    pub(super) fn task_text(
        &self,
        expected: &TaskRevision,
        id: &TaskId,
        field: TaskField,
        offset: ByteOffset,
        limit: usize,
    ) -> Result<TaskText, RpcError> {
        self.task_text_in(None, expected, id, field, offset, limit)
    }

    pub(super) fn task_text_in(
        &self,
        context: Option<&WindowContext>,
        expected: &TaskRevision,
        id: &TaskId,
        field: TaskField,
        offset: ByteOffset,
        limit: usize,
    ) -> Result<TaskText, RpcError> {
        let rows = self.task_rows_in(context)?;
        require_revision(Some(expected), &self.task_revision_in(context, &rows)?)?;
        let value = find(self, &rows, id)?
            .field(self, field)
            .ok_or_else(|| failure("this task has no readable scalar for the requested field"))?;
        let start = usize::try_from(offset.get())
            .ok()
            .filter(|offset| *offset <= value.len())
            .ok_or_else(|| failure("task text offset exceeds the accepted field"))?;
        let start = floor_utf8(&value, start);
        let end = floor_utf8(
            &value,
            start.saturating_add(limit.clamp(4, 65536)).min(value.len()),
        );
        Ok(TaskText {
            session: self.session_id().clone(),
            revision: expected.clone(),
            task: id.clone(),
            field,
            offset: ByteOffset::new(start as u64),
            next_offset: ByteOffset::new(end as u64),
            complete: end == value.len(),
            text: value[start..end].into(),
        })
    }
}

fn require_revision(
    expected: Option<&TaskRevision>,
    actual: &TaskRevision,
) -> Result<(), RpcError> {
    if expected.is_some_and(|expected| expected != actual) {
        return Err(failure(
            "task view changed; read a new page before continuing",
        ));
    }
    Ok(())
}

fn find<'a, 'b>(
    session: &SessionState,
    rows: &'a [TaskRef<'b>],
    id: &TaskId,
) -> Result<&'a TaskRef<'b>, RpcError> {
    rows.iter()
        .find(|row| &row.id(session) == id)
        .ok_or_else(|| failure("retired or foreign task"))
}

fn floor_utf8(text: &str, mut offset: usize) -> usize {
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn failure(message: &str) -> RpcError {
    RpcError {
        code: -32012,
        message: message.into(),
    }
}
