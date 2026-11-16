//! Assemble one captured task's named scalar fields without choosing a platform's display policy.

use carlos_proto::commands::RequestId;
use carlos_proto::ids::{Sequence, TaskId, TaskRevision, TaskWindowId};
use carlos_proto::tasks::{TaskDetail, TaskField, TaskText};

use crate::ClientError;

/// Preserve the native metadata view's existing byte allowance.
pub const MAX_TASK_DETAIL_BYTES: usize = 8 * 1024 * 1024;

/// Freeze the selected task and revision inside either the latest or an older captured window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskDetailTarget {
    pub window: Option<TaskWindowId>,
    pub expected: TaskRevision,
    pub task: TaskId,
}

/// Ask for a field's prefix; a platform can request the full native allowance or a shorter display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskFieldRead {
    pub field: TaskField,
    pub bytes: usize,
}

pub(crate) struct Pending {
    pub id: RequestId,
    pub field: Option<TaskField>,
    pub offset: usize,
    pub sequence: Sequence,
}

/// One checked detail and one outstanding point read, independent of widgets and stop consent.
pub struct TaskDetails {
    pub(crate) target: TaskDetailTarget,
    pub(crate) value: Option<TaskDetail>,
    pub(crate) fields: Option<Vec<TaskFieldRead>>,
    pub(crate) pending: Option<Pending>,
    pub(crate) error: Option<String>,
    pub(crate) failed_sequence: Option<Sequence>,
    pub(crate) stale: bool,
    pub(crate) dirty: bool,
}

impl TaskDetails {
    pub(crate) fn new(target: TaskDetailTarget) -> Self {
        Self {
            target,
            value: None,
            fields: None,
            pending: None,
            error: None,
            failed_sequence: None,
            stale: false,
            dirty: true,
        }
    }

    /// Borrow the exact captured target rather than infer it from the latest task header.
    pub fn target(&self) -> &TaskDetailTarget {
        &self.target
    }

    /// Borrow the checked fields; incomplete native values remain explicitly incomplete.
    pub fn value(&self) -> Option<&TaskDetail> {
        self.value.as_ref()
    }

    /// Borrow a retained field without copying its text or claiming an unfinished value is complete.
    pub fn field(&self, field: TaskField) -> Option<&str> {
        self.value
            .as_ref()?
            .properties
            .iter()
            .find(|property| property.field == field)
            .map(|property| property.text.as_str())
    }

    /// Report whether the platform has chosen which named fields it wants to read.
    pub fn has_field_interest(&self) -> bool {
        self.fields.is_some()
    }

    /// Distinguish an issued request from a retained or failed display.
    pub fn is_reading(&self) -> bool {
        self.pending.is_some()
    }

    /// Borrow a scoped read failure while keeping the checked previews.
    pub fn read_error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Retained older metadata stays readonly after its socket and native window retire.
    pub fn is_stale(&self) -> bool {
        self.stale
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        1024 + self.error.as_ref().map_or(0, String::len)
            + self.value.as_ref().map_or(0, |value| {
                value.task.label.len()
                    + value.task.state.len()
                    + value
                        .properties
                        .iter()
                        .map(|property| property.text.capacity() + 64)
                        .sum::<usize>()
            })
    }

    pub(crate) fn disconnected(&mut self) {
        self.pending = None;
        self.stale = self.target.window.is_some();
        self.dirty = true;
    }

    pub(crate) fn next_field(&self) -> Option<(TaskField, usize, usize)> {
        let value = self.value.as_ref()?;
        self.fields.as_ref()?.iter().find_map(|wanted| {
            let property = value
                .properties
                .iter()
                .find(|property| property.field == wanted.field)?;
            let left = wanted.bytes.saturating_sub(property.text.len());
            // A shorter display prefix can end before a multibyte character; never split it.
            let enough = left < 4 && property.bytes.get() > wanted.bytes as u64;
            (!property.complete && left > 0 && !enough).then_some((
                wanted.field,
                property.text.len(),
                left,
            ))
        })
    }

    pub(crate) fn accept_text(&mut self, read: Pending, text: TaskText) -> Result<(), ClientError> {
        let value = self.value.as_mut().ok_or(ClientError::StaleTasks)?;
        let retained: usize = value
            .properties
            .iter()
            .map(|property| property.text.len())
            .sum();
        let property = value
            .properties
            .iter_mut()
            .find(|property| Some(property.field) == read.field)
            .ok_or_else(|| ClientError::InvalidPayload("Task field disappeared".into()))?;
        if text.task != self.target.task
            || text.revision != self.target.expected
            || Some(text.field) != read.field
            || text.offset.get() != read.offset as u64
            || property.text.len() != read.offset
            || text.next_offset.get() > property.bytes.get()
            || text.complete != (text.next_offset.get() == property.bytes.get())
            || retained.saturating_add(text.text.len()) > MAX_TASK_DETAIL_BYTES
        {
            return Err(ClientError::InvalidPayload(
                "Task field changed its exact byte interval or exceeded the native view".into(),
            ));
        }
        property.text.push_str(&text.text);
        property.complete = text.complete;
        Ok(())
    }
}
