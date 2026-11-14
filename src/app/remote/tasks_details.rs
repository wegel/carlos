//! Read one selected task's displayed scalar fields without retaining native metadata objects.

use anyhow::Result;
use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::{Sequence, TaskId, TaskRevision};
use carlos_proto::tasks::{TaskDetail, TaskField, TaskKind, TaskText};

use super::transport::Transport;
use super::Device;

pub(super) const MAX_DETAIL_BYTES: usize = 8 * 1024 * 1024;

struct DetailRead {
    id: RequestId,
    task: TaskId,
    revision: TaskRevision,
    sequence: Sequence,
}

struct TextRead {
    id: RequestId,
    field: TaskField,
    offset: usize,
    sequence: Sequence,
}

/// Only one selected task and one named text request remain live on this device.
#[derive(Default)]
pub(super) struct Details {
    pub(super) value: Option<TaskDetail>,
    pub(super) error: Option<String>,
    failed_sequence: Option<Sequence>,
    loading: Option<DetailRead>,
    text: Option<TextRead>,
}

impl Details {
    pub(super) fn clear(&mut self) -> Vec<RequestId> {
        let mut retired = Vec::new();
        retired.extend(self.loading.take().map(|read| read.id));
        retired.extend(self.text.take().map(|read| read.id));
        self.value = None;
        self.error = None;
        self.failed_sequence = None;
        retired
    }

    pub(super) fn disconnected(&mut self) {
        self.loading = None;
        self.text = None;
    }

    pub(super) fn pending(&self) -> bool {
        self.loading.is_some() || self.text.is_some()
    }

    pub(super) fn field(&self, field: TaskField) -> Option<&str> {
        self.value
            .as_ref()?
            .properties
            .iter()
            .find(|property| property.field == field)
            .map(|property| {
                let text = property.text.as_str();
                if property.complete
                    || text.len()
                        >= display_limit(
                            self.value.as_ref().expect("selected detail").task.kind,
                            field,
                        )
                {
                    return text;
                }
                let mut end = text.len().min(2048);
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                &text[..end]
            })
    }

    pub(super) fn failed(&mut self, id: &RequestId, message: &str, current: Sequence) -> bool {
        let sequence = if self.loading.as_ref().is_some_and(|read| &read.id == id) {
            self.loading.take().map(|read| read.sequence)
        } else if self.text.as_ref().is_some_and(|read| &read.id == id) {
            self.text.take().map(|read| read.sequence)
        } else {
            return false;
        };
        if sequence.is_some_and(|sequence| sequence >= current) {
            self.failed_sequence = sequence;
            self.error = Some(message.into());
        }
        true
    }

    fn retire_failure(&mut self, current: Sequence) {
        if self.failed_sequence.is_some_and(|old| old < current) {
            self.failed_sequence = None;
            self.error = None;
        }
    }
}

impl Device {
    pub(super) fn clear_task_details(&mut self) {
        let retired = self.tasks.details.clear();
        self.tasks.reads.retire(retired);
    }

    pub(super) fn drive_task_details(&mut self, link: &mut Transport) -> Result<()> {
        let sequence = self.task_sequence();
        self.tasks.details.retire_failure(sequence);
        let Some(task) = self.tasks.pages.selected(self.tasks.view.selected.as_ref()) else {
            return Ok(());
        };
        let task = task.id.clone();
        let revision = self
            .tasks
            .pages
            .revision
            .clone()
            .expect("accepted task page");
        if self
            .tasks
            .details
            .value
            .as_ref()
            .is_some_and(|value| value.task.id != task || value.revision != revision)
        {
            self.clear_task_details();
        }
        if self.tasks.details.pending() || self.tasks.details.error.is_some() {
            return Ok(());
        }
        if self.tasks.details.value.is_some() {
            return self.read_task_field(link);
        }
        let outgoing = self.client.read(
            link.connection,
            Command::TaskDetail {
                session: self.session.clone(),
                expected: revision.clone(),
                task: task.clone(),
            },
        )?;
        self.tasks.details.loading = Some(DetailRead {
            id: outgoing.id(),
            task,
            revision,
            sequence,
        });
        link.send(outgoing)
    }

    pub(super) fn accept_task_detail(
        &mut self,
        id: Option<RequestId>,
        mut value: TaskDetail,
    ) -> bool {
        if !self
            .tasks
            .details
            .loading
            .as_ref()
            .is_some_and(|read| Some(&read.id) == id.as_ref())
        {
            return false;
        }
        let read = self
            .tasks
            .details
            .loading
            .take()
            .expect("matching selected detail");
        if read.task != value.task.id
            || read.revision != value.revision
            || self.tasks.view.selected.as_ref() != Some(&read.task)
            || self
                .task_facts()
                .is_none_or(|facts| facts.revision != read.revision)
        {
            return true;
        }
        value
            .properties
            .retain(|property| displayed(value.task.kind, property.field));
        self.tasks.details.value = Some(value);
        self.tasks.details.error = None;
        true
    }

    fn read_task_field(&mut self, link: &mut Transport) -> Result<()> {
        let value = self
            .tasks
            .details
            .value
            .as_ref()
            .expect("selected metadata");
        let Some(property) = value.properties.iter().find(|property| {
            !property.complete
                && property.text.len() < display_limit(value.task.kind, property.field)
        }) else {
            return Ok(());
        };
        let bytes: usize = value
            .properties
            .iter()
            .map(|property| property.text.len())
            .sum();
        if property.bytes.get() > MAX_DETAIL_BYTES as u64 || bytes >= MAX_DETAIL_BYTES {
            self.tasks.details.error = Some(
                "Task detail exceeds the native 8 MiB view; existing previews are kept".into(),
            );
            return Ok(());
        }
        let limit = 65536.min(MAX_DETAIL_BYTES - bytes).min(
            display_limit(value.task.kind, property.field).saturating_sub(property.text.len()),
        );
        if limit < 4 {
            self.tasks.details.error = Some("Task detail reached the native view limit".into());
            return Ok(());
        }
        let outgoing = self.client.read(
            link.connection,
            Command::TaskText {
                session: self.session.clone(),
                expected: value.revision.clone(),
                task: value.task.id.clone(),
                field: property.field,
                offset: ByteOffset::new(property.text.len() as u64),
                limit: Some(limit as u32),
            },
        )?;
        self.tasks.details.text = Some(TextRead {
            id: outgoing.id(),
            field: property.field,
            offset: property.text.len(),
            sequence: self.task_sequence(),
        });
        link.send(outgoing)
    }

    pub(super) fn accept_task_text(&mut self, id: Option<RequestId>, text: TaskText) -> bool {
        if !self
            .tasks
            .details
            .text
            .as_ref()
            .is_some_and(|read| Some(&read.id) == id.as_ref())
        {
            return false;
        }
        let read = self.tasks.details.text.take().expect("matching task field");
        if self
            .task_facts()
            .is_none_or(|facts| facts.revision != text.revision)
        {
            return true;
        }
        let Some(value) = self.tasks.details.value.as_mut() else {
            return true;
        };
        if value.task.id != text.task
            || value.revision != text.revision
            || self.tasks.view.selected.as_ref() != Some(&text.task)
        {
            return true;
        }
        let Some(property) = value
            .properties
            .iter_mut()
            .find(|property| property.field == read.field)
        else {
            return true;
        };
        if text.offset.get() != read.offset as u64
            || property.text.len() != read.offset
            || text.next_offset.get() > property.bytes.get()
            || text.complete != (text.next_offset.get() == property.bytes.get())
        {
            self.tasks.details.error =
                Some("Task field changed its exact byte interval; r rereads it".into());
        } else {
            property.text.push_str(&text.text);
            property.complete = text.complete;
        }
        true
    }
}

fn displayed(kind: TaskKind, field: TaskField) -> bool {
    field != TaskField::Label && (kind != TaskKind::Agent || field != TaskField::Description)
}

pub(super) fn display_limit(kind: TaskKind, field: TaskField) -> usize {
    if kind == TaskKind::Agent && field == TaskField::Report {
        65536
    } else {
        MAX_DETAIL_BYTES
    }
}
