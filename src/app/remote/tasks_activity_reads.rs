//! Read child changes and frozen display pages without a client polling clock or native JSON.

use anyhow::{ensure, Result};
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::task_activity::{TaskActivityHeader, TaskActivityPage};

use super::tasks_activity::Display;
use super::transport::Transport;
use super::Device;

impl Device {
    pub(super) fn drive_task_activity(&mut self, link: &mut Transport) -> Result<bool> {
        self.flush_activity_outgoing(link)?;
        if !self.tasks.activity.show
            || self.tasks.activity.stale
            || !self.client.session_is_ready(link.connection, &self.session)
        {
            return Ok(false);
        }
        let Some(header) = self.tasks.activity.header.clone() else {
            return Ok(false);
        };
        if self.tasks.activity.requests.header.is_none() {
            let outgoing = self.client.read(
                link.connection,
                Command::TaskActivityRead {
                    session: self.session.clone(),
                    activity: header.activity.clone(),
                    since: Some(header.change),
                },
            )?;
            self.tasks.activity.requests.header = Some(outgoing.id());
            link.send(outgoing)?;
        }
        self.read_activity_display(link, header)
    }

    pub(super) fn flush_activity_outgoing(&mut self, link: &mut Transport) -> Result<()> {
        for outgoing in std::mem::take(&mut self.tasks.activity.outgoing) {
            link.send(outgoing)?;
        }
        if std::mem::take(&mut self.tasks.activity.close) {
            link.send(self.client.read(
                link.connection,
                Command::TaskActivityClose {
                    session: self.session.clone(),
                    activity: None,
                },
            )?)?;
        }
        Ok(())
    }

    fn read_activity_display(
        &mut self,
        link: &mut Transport,
        header: TaskActivityHeader,
    ) -> Result<bool> {
        if !header.known
            || header.reading
            || header.error.is_some()
            || self.tasks.activity.requests.page.is_some()
            || self.tasks.activity.requests.advance.is_some()
            || self.tasks.activity.failed_revision.as_ref() == Some(&header.revision)
            || self
                .tasks
                .activity
                .accepted
                .as_ref()
                .is_some_and(|display| display.revision == header.revision)
        {
            return Ok(false);
        }
        if self
            .tasks
            .activity
            .pending
            .as_ref()
            .is_none_or(|display| display.revision != header.revision)
        {
            self.tasks.activity.pending = Some(Display {
                revision: header.revision.clone(),
                records: Vec::new(),
                bytes: 0,
                more: true,
            });
        }
        let after = self
            .tasks
            .activity
            .pending
            .as_ref()
            .and_then(|display| display.records.last())
            .map(|record| record.id.clone());
        let outgoing = self.client.read(
            link.connection,
            Command::TaskActivityPage {
                session: self.session.clone(),
                activity: header.activity,
                expected: header.revision,
                after,
                limit: Some(128),
            },
        )?;
        self.tasks.activity.requests.page = Some(outgoing.id());
        link.send(outgoing)?;
        Ok(true)
    }

    pub(super) fn accept_activity_header(
        &mut self,
        id: Option<&RequestId>,
        header: TaskActivityHeader,
    ) -> bool {
        let requests = &mut self.tasks.activity.requests;
        let matched = if id.is_some() && requests.header.as_ref() == id {
            requests.header = None;
            true
        } else if id.is_some() && requests.advance.as_ref() == id {
            requests.advance = None;
            true
        } else {
            false
        };
        if !matched {
            return false;
        }
        if self.tasks.activity.task.as_ref() != Some(&header.task) {
            self.tasks.activity.error =
                Some("Child reply changed its original task; no reader was redirected".into());
            self.tasks.activity.stale = true;
            return true;
        }
        if self
            .tasks
            .activity
            .header
            .as_ref()
            .is_some_and(|current| current.change > header.change)
        {
            return true;
        }
        self.tasks.activity.stale |= header.closed;
        self.tasks.activity.header = Some(header);
        self.tasks.paint_dirty.set(true);
        true
    }

    pub(super) fn accept_activity_page(
        &mut self,
        id: Option<&RequestId>,
        page: TaskActivityPage,
    ) -> Result<bool> {
        if id.is_none() || self.tasks.activity.requests.page.as_ref() != id {
            return Ok(false);
        }
        self.tasks.activity.requests.page = None;
        if self.tasks.activity.header.as_ref().is_none_or(|header| {
            header.activity != page.activity || header.revision != page.revision || header.reading
        }) {
            self.tasks.activity.pending = None;
            return Ok(true);
        }
        let Some(display) = &mut self.tasks.activity.pending else {
            return Ok(true);
        };
        for mut record in page.records {
            ensure!(
                !display.records.iter().any(|old| old.id == record.id),
                "child page repeated an earlier native item"
            );
            if display.bytes >= 128 * 1024 {
                break;
            }
            let remaining = 128 * 1024 - display.bytes;
            if record.text.len() > remaining {
                let mut end = remaining;
                while !record.text.is_char_boundary(end) {
                    end -= 1;
                }
                record.text.truncate(end);
                record.complete = false;
            }
            display.bytes += record.text.len();
            display.records.push(record);
        }
        display.more = page.has_more && display.bytes < 128 * 1024;
        if !display.more {
            self.tasks.activity.accepted = self.tasks.activity.pending.take();
            self.tasks.activity.error = None;
            self.tasks.paint_dirty.set(true);
        }
        Ok(true)
    }

    pub(super) fn fail_task_activity(&mut self, id: &RequestId, error: &str) -> bool {
        let requests = &mut self.tasks.activity.requests;
        if requests.page.as_ref() == Some(id) {
            requests.page = None;
            self.tasks.activity.failed_revision = self
                .tasks
                .activity
                .pending
                .take()
                .map(|display| display.revision);
        } else if requests.header.as_ref() == Some(id) || requests.advance.as_ref() == Some(id) {
            if requests.header.as_ref() == Some(id) {
                requests.header = None;
            }
            if requests.advance.as_ref() == Some(id) {
                requests.advance = None;
            }
            self.tasks.activity.stale = true;
        } else {
            return false;
        }
        self.tasks.activity.error = Some(format!(
            "Child activity read failed; accepted display kept: {error}"
        ));
        self.tasks.paint_dirty.set(true);
        true
    }
}
