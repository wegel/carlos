//! Drive one bounded task page against the Client's accepted header without a terminal native timer.

use std::collections::VecDeque;

use anyhow::Result;
use carlos_client::ClientError;
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::{Sequence, TaskId, TaskRevision};
use carlos_proto::tasks::TaskPage;

use super::transport::Transport;
use super::Device;

pub(super) struct PageRead {
    pub(super) id: RequestId,
    start: usize,
    sequence: Sequence,
}
struct Failure {
    sequence: Sequence,
    text: String,
}

/// Keep one outstanding page and at most one future page while numbered facts catch up.
#[derive(Default)]
pub(super) struct Reads {
    pub(super) header: Option<RequestId>,
    pub(super) header_error: Option<String>,
    pub(super) waiting: Option<Sequence>,
    pub(super) request: Option<PageRead>,
    pub(super) future: Option<(PageRead, TaskPage)>,
    failure: Option<Failure>,
    retired: VecDeque<RequestId>,
}

impl Reads {
    pub(super) fn error(&self) -> Option<&str> {
        self.header_error
            .as_deref()
            .or_else(|| self.failure.as_ref().map(|failure| failure.text.as_str()))
    }

    pub(super) fn retire(&mut self, ids: impl IntoIterator<Item = RequestId>) {
        self.retired.extend(ids);
        while self.retired.len() > 128 {
            self.retired.pop_front();
        }
    }

    pub(super) fn take_retired(&mut self, id: &RequestId) -> bool {
        if let Some(index) = self.retired.iter().position(|old| old == id) {
            self.retired.remove(index);
            true
        } else {
            false
        }
    }

    pub(super) fn page_changed(&mut self) {
        let ids = self.request.take().map(|read| read.id);
        self.retire(ids);
        self.future = None;
        self.failure = None;
    }

    pub(super) fn clear(&mut self) {
        let ids = self.request.take().map(|read| read.id);
        self.retire(ids);
        self.future = None;
        self.failure = None;
        self.header_error = None;
        self.waiting = None;
    }

    pub(super) fn close(&mut self) {
        let header = self.header.take();
        self.retire(header);
        self.clear();
    }

    pub(super) fn disconnected(&mut self) {
        self.header = None;
        self.waiting = None;
        self.request = None;
        self.future = None;
        self.retired.clear();
    }

    pub(super) fn fail(&mut self, id: &RequestId, message: &str, current: Sequence) -> bool {
        if self.header.as_ref() == Some(id) {
            self.header = None;
            self.header_error = Some(message.into());
            return true;
        }
        if !self.request.as_ref().is_some_and(|read| &read.id == id) {
            return false;
        }
        let read = self.request.take().expect("matching task page");
        if read.sequence >= current {
            self.failure = Some(Failure {
                sequence: read.sequence,
                text: message.into(),
            });
        }

        true
    }

    fn retire_failure(&mut self, current: Sequence) {
        if self
            .failure
            .as_ref()
            .is_some_and(|failure| failure.sequence < current)
        {
            self.failure = None;
        }
    }
}

impl Device {
    pub(super) fn drive_tasks(&mut self, link: &mut Transport) -> Result<bool> {
        let changed = self.drive_task_reads(link)?;
        if changed {
            self.tasks.paint_dirty.set(true);
        }
        Ok(changed)
    }

    fn drive_task_reads(&mut self, link: &mut Transport) -> Result<bool> {
        self.drive_task_children_and_stops(link)?;
        let changed = self.settle_task_page()?;
        if self.tasks.activity.show && !self.tasks.activity.stale {
            return Ok(changed);
        }
        if !self.tasks.view.show || !self.client.session_is_ready(link.connection, &self.session) {
            return Ok(changed);
        }
        if self.tasks.reads.header.is_some()
            || self.tasks.reads.request.is_some()
            || self.tasks.reads.future.is_some()
        {
            return Ok(changed);
        }
        if self.wait_for_task_header() {
            return Ok(changed);
        }
        if self.tasks.refresh {
            self.refresh_latest_tasks(link)?;
            return Ok(true);
        }
        if self.tasks.windows.stale {
            return Ok(changed);
        }
        let Some(revision) = self
            .task_view_revision()
            .filter(|_| {
                self.tasks.windows.current.is_some()
                    || self
                        .task_facts()
                        .is_some_and(|facts| facts.known && !facts.reading)
            })
            .cloned()
        else {
            return Ok(changed);
        };
        let sequence = self.task_sequence();
        self.tasks.reads.retire_failure(sequence);
        if self.tasks.reads.error().is_some() {
            return Ok(changed);
        }
        if self.tasks.pages.revision.as_ref() != Some(&revision) {
            self.tasks
                .pages
                .reset(revision.clone(), self.tasks.view.selected.is_some());
            self.clear_task_details();
        }
        if let Some((start, after)) = self.tasks.pages.next() {
            self.request_task_page(link, Some(revision), after, start, false)?;
            return Ok(true);
        }
        self.drive_task_details(link)?;
        self.drive_task_overview(link)?;
        Ok(changed)
    }

    fn drive_task_children_and_stops(&mut self, link: &mut Transport) -> Result<()> {
        if self.client.session_is_ready(link.connection, &self.session) {
            self.drive_task_windows(link)?;
            self.drive_task_activity(link)?;
            self.restore_stale_child_list(link)?;
        }
        if let Err(error) = self.drive_task_stop(link) {
            self.tasks.view.error = Some(error.to_string());
            self.tasks.paint_dirty.set(true);
        }
        Ok(())
    }

    fn restore_stale_child_list(&mut self, link: &mut Transport) -> Result<()> {
        if self.tasks.activity.show
            && self.tasks.activity.stale
            && self.tasks.windows.current.is_none()
        {
            let effects = self.client.watch_tasks(&self.session)?;
            self.tasks.reads.header = self.client.pending_task_read(&self.session);
            for outgoing in effects.outgoing {
                link.send(outgoing)?;
            }
        }
        Ok(())
    }

    fn refresh_latest_tasks(&mut self, link: &mut Transport) -> Result<()> {
        self.tasks.refresh = false;
        let before = self.client.pending_task_read(&self.session);
        let effects = self.client.watch_tasks(&self.session)?;
        self.tasks.reads.header = self.client.pending_task_read(&self.session);
        let subscribed = self.tasks.reads.header.is_some() && self.tasks.reads.header != before;
        for outgoing in effects.outgoing {
            link.send(outgoing)?;
        }
        if !subscribed {
            self.request_task_page(link, None, None, 0, true)?;
        }
        Ok(())
    }

    fn wait_for_task_header(&mut self) -> bool {
        let Some(waiting) = self.tasks.reads.waiting else {
            return false;
        };
        if self
            .client
            .session(&self.session)
            .is_some_and(|replica| replica.sequence() < waiting)
        {
            return true;
        }
        self.tasks.reads.waiting = None;
        false
    }

    fn request_task_page(
        &mut self,
        link: &mut Transport,
        expected: Option<TaskRevision>,
        after: Option<TaskId>,
        start: usize,
        refresh: bool,
    ) -> Result<()> {
        let sequence = self.task_sequence();
        let outgoing = self.client.read(
            link.connection,
            Command::TaskList {
                session: self.session.clone(),
                window: self.task_window_id(),
                expected,
                after,
                limit: Some(32),
                refresh,
            },
        )?;
        self.tasks.reads.request = Some(PageRead {
            id: outgoing.id(),
            start,
            sequence,
        });
        link.send(outgoing)
    }

    fn settle_task_page(&mut self) -> Result<bool> {
        let Some((read, page)) = self.tasks.reads.future.take() else {
            return Ok(false);
        };
        match if self.tasks.windows.current.is_some() {
            Ok(())
        } else {
            self.client.check_task_page(&page)
        } {
            Ok(()) => {}
            Err(ClientError::StaleTasks) => {
                if self
                    .client
                    .session(&self.session)
                    .is_some_and(|replica| replica.sequence() < page.seq)
                {
                    self.tasks.reads.future = Some((read, page));
                    return Ok(false);
                }
                self.tasks.pages.revision = None;
                return Ok(true);
            }
            Err(error) => return Err(error.into()),
        }
        if !page.known || page.reading {
            return Ok(true);
        }
        let count = self.task_count();
        if self.tasks.pages.revision.as_ref() != Some(&page.revision) {
            self.tasks
                .pages
                .reset(page.revision.clone(), self.tasks.view.selected.is_some());
        }
        if let Err(error) =
            self.tasks
                .pages
                .accept(page, read.start, count, &mut self.tasks.view.selected)
        {
            self.tasks.reads.failure = Some(Failure {
                sequence: read.sequence,
                text: error.to_string(),
            });
        }
        Ok(true)
    }
}
