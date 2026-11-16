//! Keep one captured child's accepted display separate from new pages and native reader rights.

use carlos_proto::commands::RequestId;
use carlos_proto::ids::{TaskActivityId, TaskRevision};
use carlos_proto::task_activity::{
    TaskActivityHeader, TaskActivityPage, TaskActivityRecord, MAX_ACTIVITY_ITEMS,
};

use crate::ClientError;

use super::TaskDetailTarget;

/// Preserve the existing complete child display's aggregate text allowance.
pub const MAX_TASK_ACTIVITY_BYTES: usize = 128 * 1024;

pub(crate) struct Display {
    pub revision: TaskRevision,
    pub records: Vec<TaskActivityRecord>,
    pub bytes: usize,
    pub charge: usize,
}

#[derive(Default)]
pub(crate) struct ActivityRequests {
    pub header: Option<RequestId>,
    pub page: Option<RequestId>,
    pub advance: Option<RequestId>,
}

/// One device's child display and reader status, independent of terminal keys and root transcript.
#[derive(Default)]
pub struct TaskActivityPanel {
    pub(crate) target: Option<TaskDetailTarget>,
    pub(crate) header: Option<TaskActivityHeader>,
    pub(crate) accepted: Option<Display>,
    pub(crate) pending: Option<Display>,
    pub(crate) requests: ActivityRequests,
    pub(crate) stale: bool,
    pub(crate) wanted: bool,
    pub(crate) dirty: bool,
    pub(crate) error: Option<String>,
    pub(crate) failed_revision: Option<TaskRevision>,
    // Allocating opens must keep their reply ids until an exact reader id can be closed.
    // The existing 128-request allowance bounds these ids; no separate request map exists.
    pub(crate) retired: Vec<RequestId>,
    pub(crate) close: Vec<TaskActivityId>,
}

impl TaskActivityPanel {
    /// Borrow the original reader's latest checked header and live tail.
    pub fn header(&self) -> Option<&TaskActivityHeader> {
        self.header.as_ref()
    }

    /// Borrow accepted records while a reload builds its replacement separately.
    pub fn records(&self) -> &[TaskActivityRecord] {
        self.accepted
            .as_ref()
            .map_or(&[], |display| display.records.as_slice())
    }

    /// Distinguish a disconnected or failed reader from a current native interest.
    pub fn is_stale(&self) -> bool {
        self.stale
    }

    /// Report a pending explicit native reload or older-history advance.
    pub fn is_advancing(&self) -> bool {
        self.requests.advance.is_some()
    }

    /// Report an incomplete replacement display without hiding the last accepted records.
    pub fn is_reading_pages(&self) -> bool {
        self.pending.is_some()
    }

    /// Borrow a scoped reader error without changing the selected task or private draft.
    pub fn read_error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        2048 + self.error.as_ref().map_or(0, String::len)
            + self.header.as_ref().map_or(0, |header| {
                header.tail.capacity() + header.error.as_ref().map_or(0, String::len)
            })
            + self.accepted.as_ref().map_or(0, |display| display.charge)
            + self.pending.as_ref().map_or(0, |display| display.charge)
    }

    pub(crate) fn disconnected(&mut self) {
        self.requests = ActivityRequests::default();
        self.pending = None;
        self.retired.clear();
        self.close.clear();
        self.wanted = false;
        self.stale = true;
        self.dirty = true;
    }

    pub(crate) fn retire(&mut self) -> Vec<RequestId> {
        let mut ids = Vec::new();
        if let Some(id) = self.requests.header.take() {
            if self.header.is_none() {
                self.retired.push(id);
            } else {
                ids.push(id);
            }
        }
        ids.extend(self.requests.page.take());
        ids.extend(self.requests.advance.take());
        self.pending = None;
        ids
    }

    pub(crate) fn close_current(&mut self) {
        if self.wanted {
            if let Some(header) = &self.header {
                if !self.close.contains(&header.activity) {
                    self.close.push(header.activity.clone());
                }
            }
        }
        self.wanted = false;
        self.stale = true;
        self.dirty = true;
    }

    pub(crate) fn needs_page(&self) -> bool {
        self.header.as_ref().is_some_and(|header| {
            header.known
                && !header.reading
                && header.error.is_none()
                && self.requests.page.is_none()
                && self.requests.advance.is_none()
                && self.failed_revision.as_ref() != Some(&header.revision)
                && self
                    .accepted
                    .as_ref()
                    .is_none_or(|display| display.revision != header.revision)
        })
    }

    pub(crate) fn prepare_page(&mut self) {
        let header = self.header.as_ref().expect("accepted child header");
        if self
            .pending
            .as_ref()
            .is_none_or(|display| display.revision != header.revision)
        {
            self.pending = Some(Display {
                revision: header.revision.clone(),
                records: Vec::new(),
                bytes: 0,
                charge: 0,
            });
        }
    }

    pub(crate) fn accept_header(&mut self, header: TaskActivityHeader) {
        if self
            .target
            .as_ref()
            .is_none_or(|target| target.task != header.task)
        {
            self.error =
                Some("Child reply changed its original task; no reader was redirected".into());
            self.close.push(header.activity);
            self.stale = true;
        } else if self
            .header
            .as_ref()
            .is_none_or(|current| current.change <= header.change)
        {
            self.stale |= header.closed;
            self.header = Some(header);
        }
    }

    pub(crate) fn accept_page(&mut self, page: TaskActivityPage) -> Result<(), ClientError> {
        if self.header.as_ref().is_none_or(|header| {
            header.activity != page.activity || header.revision != page.revision || header.reading
        }) {
            self.pending = None;
            return Ok(());
        }
        let Some(display) = &mut self.pending else {
            return Ok(());
        };
        for mut record in page.records {
            if display.records.len() >= MAX_ACTIVITY_ITEMS
                || display.records.iter().any(|old| old.id == record.id)
            {
                return Err(ClientError::InvalidPayload(
                    "Child display repeated a record or exceeded its native count".into(),
                ));
            }
            if display.bytes >= MAX_TASK_ACTIVITY_BYTES {
                break;
            }
            let remaining = MAX_TASK_ACTIVITY_BYTES - display.bytes;
            if record.text.len() > remaining {
                let mut end = remaining;
                while !record.text.is_char_boundary(end) {
                    end -= 1;
                }
                record.text.truncate(end);
                record.complete = false;
            }
            display.bytes += record.text.len();
            display.charge += record.text.capacity()
                + record.turn.capacity()
                + record.item.capacity()
                + record.kind.capacity()
                + 128;
            display.records.push(record);
        }
        if !page.has_more || display.bytes >= MAX_TASK_ACTIVITY_BYTES {
            self.accepted = self.pending.take();
            self.error = None;
        }
        Ok(())
    }
}
