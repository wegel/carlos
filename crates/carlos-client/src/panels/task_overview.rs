//! Retain complete checked overview rows and named text without a platform countdown clock.

use carlos_proto::commands::RequestId;
use carlos_proto::ids::{TaskOverviewId, TaskRevision};
use carlos_proto::task_overview::{
    TaskLabel, TaskOverviewData, TaskOverviewEntry, TaskOverviewField, TaskOverviewPage,
    TaskOverviewText,
};

use crate::ClientError;

use super::MAX_TASK_DETAIL_BYTES;

pub(crate) struct TextRead {
    pub entry: TaskOverviewId,
    pub field: TaskOverviewField,
    pub offset: usize,
}

/// The latest task overview, independent of selection, private composer text and countdown repaint.
#[derive(Default)]
pub struct TaskOverviewPanel {
    pub(crate) revision: Option<TaskRevision>,
    pub(crate) entries: Vec<TaskOverviewEntry>,
    pub(crate) request: Option<RequestId>,
    pub(crate) text: Option<TextRead>,
    pub(crate) future: Option<TaskOverviewPage>,
    pub(crate) complete: bool,
    pub(crate) error: Option<String>,
    pub(crate) dirty: bool,
    pub(crate) bytes: usize,
    pub(crate) charge: usize,
    pub(crate) quota: bool,
}

impl TaskOverviewPanel {
    /// Borrow the checked rows without copying labels on every key or repaint.
    pub fn entries(&self) -> &[TaskOverviewEntry] {
        &self.entries
    }

    /// Borrow the accepted overview revision rather than infer it from selected detail metadata.
    pub fn revision(&self) -> Option<&TaskRevision> {
        self.revision.as_ref()
    }

    /// Borrow a read error while preserving the rows already accepted for this revision.
    pub fn read_error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Let a platform schedule countdown repaint without rescanning all overview rows per key.
    pub fn has_quota(&self) -> bool {
        self.quota
    }

    /// Report a pending page, long field or numbered reply that still awaits session facts.
    pub fn is_reading(&self) -> bool {
        self.request.is_some() || self.future.is_some()
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        512 + self.charge
            + self.error.as_ref().map_or(0, String::len)
            + self
                .future
                .as_ref()
                .map_or(0, |page| page.entries.iter().map(row_charge).sum::<usize>())
    }

    pub(crate) fn disconnected(&mut self) {
        self.request = None;
        self.text = None;
        self.future = None;
        self.dirty = true;
    }

    pub(crate) fn accept_page(&mut self, page: TaskOverviewPage) -> Result<(), ClientError> {
        let bytes: usize = page.entries.iter().map(row_bytes).sum();
        if self.entries.len() + page.entries.len() > 1153
            || self.bytes.saturating_add(bytes) > MAX_TASK_DETAIL_BYTES
            || page
                .entries
                .iter()
                .any(|entry| self.entries.iter().any(|old| old.id == entry.id))
        {
            return Err(invalid(
                "Task overview repeated a row or exceeded the native view bounds",
            ));
        }
        self.quota |= page
            .entries
            .iter()
            .any(|entry| matches!(entry.data, TaskOverviewData::Quota { .. }));
        self.bytes += bytes;
        self.charge += page.entries.iter().map(row_charge).sum::<usize>();
        self.entries.extend(page.entries);
        self.complete = !page.has_more;
        Ok(())
    }

    pub(crate) fn next_field(&self) -> Result<Option<TextRead>, ClientError> {
        for entry in &self.entries {
            for field in [TaskOverviewField::Label, TaskOverviewField::Message] {
                if let Some(label) = label(&entry.data, field).filter(|label| !label.complete) {
                    if label.bytes.get() > MAX_TASK_DETAIL_BYTES as u64
                        || self.bytes >= MAX_TASK_DETAIL_BYTES
                    {
                        return Err(invalid(
                            "Task overview exceeds the native 8 MiB view; previews are kept",
                        ));
                    }
                    return Ok(Some(TextRead {
                        entry: entry.id.clone(),
                        field,
                        offset: label.text.len(),
                    }));
                }
            }
        }
        Ok(None)
    }

    pub(crate) fn remaining_bytes(&self) -> usize {
        MAX_TASK_DETAIL_BYTES.saturating_sub(self.bytes)
    }

    pub(crate) fn accept_text(
        &mut self,
        read: TextRead,
        text: TaskOverviewText,
    ) -> Result<(), ClientError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.id == read.entry)
            .ok_or_else(|| invalid("Task overview row disappeared"))?;
        let label = label_mut(&mut entry.data, read.field)
            .ok_or_else(|| invalid("Task overview field disappeared"))?;
        if Some(&text.revision) != self.revision.as_ref()
            || text.entry != read.entry
            || text.field != read.field
            || text.offset.get() != read.offset as u64
            || label.text.len() != read.offset
            || text.next_offset.get() > label.bytes.get()
            || text.complete != (text.next_offset.get() == label.bytes.get())
            || self.bytes.saturating_add(text.text.len()) > MAX_TASK_DETAIL_BYTES
        {
            return Err(invalid(
                "Task overview field changed its exact byte interval or exceeded the native view",
            ));
        }
        let capacity = label.text.capacity();
        label.text.push_str(&text.text);
        label.complete = text.complete;
        self.bytes += text.text.len();
        self.charge += label.text.capacity() - capacity;
        Ok(())
    }
}

fn row_bytes(entry: &TaskOverviewEntry) -> usize {
    [TaskOverviewField::Label, TaskOverviewField::Message]
        .into_iter()
        .filter_map(|field| label(&entry.data, field))
        .map(|label| label.text.len())
        .sum()
}

fn row_charge(entry: &TaskOverviewEntry) -> usize {
    512 + [TaskOverviewField::Label, TaskOverviewField::Message]
        .into_iter()
        .filter_map(|field| label(&entry.data, field))
        .map(|label| label.text.capacity())
        .sum::<usize>()
}

fn label(data: &TaskOverviewData, field: TaskOverviewField) -> Option<&TaskLabel> {
    match (data, field) {
        (TaskOverviewData::Main { transient, .. }, TaskOverviewField::Label) => transient.as_ref(),
        (
            TaskOverviewData::Quota { label, .. }
            | TaskOverviewData::Prompt { label, .. }
            | TaskOverviewData::ProcessError { label, .. },
            TaskOverviewField::Label,
        ) => Some(label),
        (TaskOverviewData::ProcessError { message, .. }, TaskOverviewField::Message) => {
            Some(message)
        }
        _ => None,
    }
}

fn label_mut(data: &mut TaskOverviewData, field: TaskOverviewField) -> Option<&mut TaskLabel> {
    match (data, field) {
        (TaskOverviewData::Main { transient, .. }, TaskOverviewField::Label) => transient.as_mut(),
        (
            TaskOverviewData::Quota { label, .. }
            | TaskOverviewData::Prompt { label, .. }
            | TaskOverviewData::ProcessError { label, .. },
            TaskOverviewField::Label,
        ) => Some(label),
        (TaskOverviewData::ProcessError { message, .. }, TaskOverviewField::Message) => {
            Some(message)
        }
        _ => None,
    }
}

fn invalid(message: &str) -> ClientError {
    ClientError::InvalidPayload(message.into())
}
