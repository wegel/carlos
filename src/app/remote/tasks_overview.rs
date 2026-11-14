//! Read only bounded task-panel activity, quota and accepted-input display labels.

use std::time::{Duration, Instant};

use anyhow::Result;
use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::{TaskOverviewId, TaskRevision};
use carlos_proto::task_overview::{
    TaskLabel, TaskOverviewData, TaskOverviewEntry, TaskOverviewField, TaskOverviewPage,
    TaskOverviewText,
};

use super::tasks_details::MAX_DETAIL_BYTES;
use super::transport::Transport;
use super::Device;

struct PageRead {
    id: RequestId,
    revision: TaskRevision,
}
struct TextRead {
    id: RequestId,
    entry: TaskOverviewId,
    field: TaskOverviewField,
    offset: usize,
}

/// Retain bounded overview rows independently of the selected task and composer.
#[derive(Default)]
pub(super) struct Overview {
    pub(super) revision: Option<TaskRevision>,
    pub(super) entries: Vec<TaskOverviewEntry>,
    pub(super) error: Option<String>,
    request: Option<PageRead>,
    text: Option<TextRead>,
    future: Option<TaskOverviewPage>,
    complete: bool,
    next_draw: Option<Instant>,
}

impl Overview {
    pub(super) fn clear(&mut self) -> Vec<RequestId> {
        let mut retired = Vec::new();
        retired.extend(self.request.take().map(|read| read.id));
        retired.extend(self.text.take().map(|read| read.id));
        *self = Self::default();
        retired
    }

    pub(super) fn disconnected(&mut self) {
        self.request = None;
        self.text = None;
        self.future = None;
    }

    pub(super) fn failed(&mut self, id: &RequestId, message: &str) -> bool {
        if self.request.as_ref().is_some_and(|read| &read.id == id)
            || self.text.as_ref().is_some_and(|read| &read.id == id)
        {
            self.request = None;
            self.text = None;
            self.error = Some(message.into());
            true
        } else {
            false
        }
    }

    pub(super) fn tick(&mut self, now: Instant) -> bool {
        if self
            .entries
            .iter()
            .any(|entry| matches!(entry.data, TaskOverviewData::Quota { .. }))
            && self.next_draw.is_none_or(|deadline| deadline <= now)
        {
            self.next_draw = Some(now + Duration::from_secs(60));
            true
        } else {
            false
        }
    }
}

impl Device {
    pub(super) fn drive_task_overview(&mut self, link: &mut Transport) -> Result<()> {
        let Some(revision) = self
            .task_facts()
            .map(|facts| facts.overview_revision.clone())
        else {
            return Ok(());
        };
        if self.tasks.overview.revision.as_ref() != Some(&revision) {
            let retired = self.tasks.overview.clear();
            self.tasks.reads.retire(retired);
            self.tasks.overview.revision = Some(revision.clone());
        }
        self.settle_task_overview();
        let overview = &self.tasks.overview;
        if overview.error.is_some()
            || overview.request.is_some()
            || overview.text.is_some()
            || overview.future.is_some()
        {
            return Ok(());
        }
        if overview.complete {
            return self.read_task_overview_field(link);
        }
        let outgoing = self.client.read(
            link.connection,
            Command::TaskOverview {
                session: self.session.clone(),
                expected: Some(revision.clone()),
                after: overview.entries.last().map(|entry| entry.id.clone()),
                limit: Some(32),
            },
        )?;
        self.tasks.overview.request = Some(PageRead {
            id: outgoing.id(),
            revision,
        });
        link.send(outgoing)
    }

    pub(super) fn accept_task_overview(
        &mut self,
        id: Option<RequestId>,
        page: TaskOverviewPage,
    ) -> bool {
        if !self
            .tasks
            .overview
            .request
            .as_ref()
            .is_some_and(|read| Some(&read.id) == id.as_ref())
        {
            return false;
        }
        let read = self
            .tasks
            .overview
            .request
            .take()
            .expect("matching overview page");
        if read.revision == page.revision {
            self.tasks.overview.future = Some(page);
        }
        true
    }

    fn settle_task_overview(&mut self) {
        let Some(page) = self.tasks.overview.future.take() else {
            return;
        };
        if self
            .task_facts()
            .is_none_or(|facts| facts.overview_revision != page.revision)
        {
            return;
        }
        if self
            .client
            .session(&self.session)
            .is_some_and(|replica| replica.sequence() < page.seq)
        {
            self.tasks.overview.future = Some(page);
            return;
        }
        let overview = &mut self.tasks.overview;
        if overview.entries.len() + page.entries.len() > 1153
            || page
                .entries
                .iter()
                .any(|entry| overview.entries.iter().any(|old| old.id == entry.id))
        {
            overview.error =
                Some("Task overview repeated a row or exceeded the native view bounds".into());
            return;
        }
        overview.entries.extend(page.entries);
        overview.complete = !page.has_more;
    }

    fn read_task_overview_field(&mut self, link: &mut Transport) -> Result<()> {
        let overview = &self.tasks.overview;
        let Some((entry, field, label)) = incomplete_label(&overview.entries) else {
            return Ok(());
        };
        let retained = retained_bytes(&overview.entries);
        if label.bytes.get() > MAX_DETAIL_BYTES as u64 || retained >= MAX_DETAIL_BYTES {
            self.tasks.overview.error =
                Some("Task overview exceeds the native 8 MiB view; previews are kept".into());
            return Ok(());
        }
        let limit = 65536.min(MAX_DETAIL_BYTES - retained);
        if limit < 4 {
            self.tasks.overview.error = Some("Task overview reached its native view limit".into());
            return Ok(());
        }
        let outgoing = self.client.read(
            link.connection,
            Command::TaskOverviewText {
                session: self.session.clone(),
                expected: overview.revision.clone().expect("overview revision"),
                entry: entry.clone(),
                field,
                offset: ByteOffset::new(label.text.len() as u64),
                limit: Some(limit as u32),
            },
        )?;
        self.tasks.overview.text = Some(TextRead {
            id: outgoing.id(),
            entry: entry.clone(),
            field,
            offset: label.text.len(),
        });
        link.send(outgoing)
    }

    pub(super) fn accept_task_overview_text(
        &mut self,
        id: Option<RequestId>,
        text: TaskOverviewText,
    ) -> bool {
        if !self
            .tasks
            .overview
            .text
            .as_ref()
            .is_some_and(|read| Some(&read.id) == id.as_ref())
        {
            return false;
        }
        let read = self
            .tasks
            .overview
            .text
            .take()
            .expect("matching overview text");
        let overview = &mut self.tasks.overview;
        if overview.revision.as_ref() != Some(&text.revision) {
            return true;
        }
        let Some(entry) = overview
            .entries
            .iter_mut()
            .find(|entry| entry.id == read.entry)
        else {
            return true;
        };
        let Some(label) = label_mut(&mut entry.data, read.field) else {
            return true;
        };
        if text.offset.get() != read.offset as u64
            || label.text.len() != read.offset
            || text.next_offset.get() > label.bytes.get()
            || text.complete != (text.next_offset.get() == label.bytes.get())
        {
            overview.error =
                Some("Task overview field changed its exact byte interval; r rereads it".into());
        } else {
            label.text.push_str(&text.text);
            label.complete = text.complete;
        }
        true
    }

    pub(super) fn tick_task_countdowns(&mut self) -> bool {
        let changed = self.tasks.view.show && self.tasks.overview.tick(Instant::now());
        if changed {
            self.tasks.paint_dirty.set(true);
        }
        changed
    }
}

fn incomplete_label(
    entries: &[TaskOverviewEntry],
) -> Option<(&TaskOverviewId, TaskOverviewField, &TaskLabel)> {
    for entry in entries {
        for field in [TaskOverviewField::Label, TaskOverviewField::Message] {
            if let Some(label) = label(&entry.data, field).filter(|label| !label.complete) {
                return Some((&entry.id, field, label));
            }
        }
    }
    None
}

fn retained_bytes(entries: &[TaskOverviewEntry]) -> usize {
    entries
        .iter()
        .map(|entry| {
            [TaskOverviewField::Label, TaskOverviewField::Message]
                .into_iter()
                .filter_map(|field| label(&entry.data, field))
                .map(|label| label.text.len())
                .sum::<usize>()
        })
        .sum()
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
