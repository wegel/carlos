//! Correlate bounded queue pages with one accepted client fact copy and one local preview window.

use std::time::Instant;

use anyhow::Result;
use carlos_client::{ClientError, Effects, ReadReply, RequestFailure};
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::{QueueEntryId, QueueRevision};
use carlos_proto::queue::{QueuePage, MAX_PAGE_ENTRIES};

use super::transport::Transport;
use super::Device;

/// Retain one page address while its typed read completes.
pub(super) struct PageRead {
    id: RequestId,
    start: usize,
    refresh: bool,
    revision: Option<QueueRevision>,
}

/// Hold at most one reply that arrived before its numbered session facts.
#[derive(Default)]
pub(super) struct Reads {
    request: Option<PageRead>,
    future: Option<(PageRead, QueuePage)>,
    failure: Option<ReadFailure>,
}

struct ReadFailure {
    revision: Option<QueueRevision>,
    message: String,
}

impl Reads {
    /// Borrow only this page read's error, leaving editor and operation failures independent.
    pub(super) fn error(&self) -> Option<&str> {
        self.failure
            .as_ref()
            .map(|failure| failure.message.as_str())
    }

    /// Retry after an explicit navigation or refresh without clearing an editor failure.
    pub(super) fn clear_failure(&mut self) {
        self.failure = None;
    }

    fn retire_failure(&mut self, revision: &QueueRevision) -> bool {
        if self
            .failure
            .as_ref()
            .is_some_and(|failure| failure.revision.as_ref().is_some_and(|old| old != revision))
        {
            self.failure = None;
            return true;
        }
        false
    }

    /// Keep the previous page visibly stale until its requested replacement has settled.
    pub(super) fn busy(&self) -> bool {
        self.request.is_some() || self.future.is_some()
    }

    /// Retry an unfinished explicit refresh without invalidating an unchanged editor on every EOF.
    pub(super) fn refreshing(&self) -> bool {
        self.request.as_ref().is_some_and(|read| read.refresh)
            || self.future.as_ref().is_some_and(|(read, _)| read.refresh)
    }
}

impl Device {
    /// Apply only queue replies owned by the current local page or full-text read.
    pub(super) fn apply_queue_reads(&mut self, effects: &mut Effects) -> Result<bool> {
        let mut changed = false;
        for result in std::mem::take(&mut effects.replies) {
            match result.reply {
                ReadReply::Queue(page) => {
                    if self
                        .queue
                        .reads
                        .request
                        .as_ref()
                        .is_some_and(|read| Some(&read.id) == result.id.as_ref())
                    {
                        let read = self
                            .queue
                            .reads
                            .request
                            .take()
                            .expect("matching queue page");
                        self.queue.reads.future = Some((read, page));
                        changed = true;
                    }
                }
                ReadReply::QueueText(text) => changed |= self.accept_queue_text(result.id, text)?,
                _ => effects.replies.push(result),
            }
        }
        for failure in std::mem::take(&mut effects.failures) {
            let page_failed = self.fail_queue_page(&failure);
            let text_failed = self.fail_queue_text(&failure);
            changed |= page_failed || text_failed;
            if !(page_failed || text_failed) {
                effects.failures.push(failure);
            }
        }
        Ok(changed)
    }

    fn fail_queue_page(&mut self, failure: &RequestFailure) -> bool {
        if !self
            .queue
            .reads
            .request
            .as_ref()
            .is_some_and(|read| Some(&read.id) == failure.id.as_ref())
        {
            return false;
        }
        let read = self
            .queue
            .reads
            .request
            .take()
            .expect("matching failed page");
        self.queue.reads.failure = Some(ReadFailure {
            revision: read.revision,
            message: failure.error.message.clone(),
        });
        true
    }

    /// Start only the next needed bounded read; native invalidation and timers stay in the daemon.
    pub(super) fn drive_queue(&mut self, link: &mut Transport) -> Result<bool> {
        let mut changed = self.settle_queue_page(link)?;
        if !self.client.is_ready(link.connection) {
            return Ok(changed);
        }
        if !(self.view.codex_queue.show || self.view.codex_queue.mode)
            || !self.client.session_is_ready(link.connection, &self.session)
        {
            return Ok(changed);
        }
        if self.queue.reads.request.is_some() || self.queue.reads.future.is_some() {
            return Ok(changed);
        }
        if self.queue.refresh {
            self.queue.refresh = false;
            self.request_queue_page(link, None, None, 0, true)?;
            return Ok(true);
        }
        let Some(facts) = self
            .queue_facts()
            .filter(|facts| facts.known && !facts.reading)
        else {
            return Ok(changed);
        };
        let revision = facts.revision.clone();
        changed |= self.queue.reads.retire_failure(&revision);
        if self.queue.reads.error().is_some() {
            return Ok(changed);
        }
        if self.queue.pages.revision.as_ref() != Some(&revision) {
            self.queue
                .pages
                .reset(revision.clone(), self.view.codex_queue.selected.is_some());
            changed = true;
        }
        if let Some((start, after)) = self.queue.pages.next() {
            self.request_queue_page(link, Some(revision), after, start, false)?;
        } else {
            self.drive_queue_text(link)?;
        }
        Ok(changed)
    }

    fn request_queue_page(
        &mut self,
        link: &mut Transport,
        expected: Option<QueueRevision>,
        after: Option<QueueEntryId>,
        start: usize,
        refresh: bool,
    ) -> Result<()> {
        let revision = expected
            .clone()
            .or_else(|| self.queue_facts().map(|facts| facts.revision.clone()));
        let outgoing = self.client.read(
            link.connection,
            Command::QueueRead {
                session: self.session.clone(),
                expected,
                after,
                limit: Some(MAX_PAGE_ENTRIES as u16),
                refresh,
            },
        )?;
        self.queue.reads.request = Some(PageRead {
            id: outgoing.id(),
            start,
            refresh,
            revision,
        });
        link.send(outgoing)
    }

    fn settle_queue_page(&mut self, link: &mut Transport) -> Result<bool> {
        let Some((read, page)) = self.queue.reads.future.take() else {
            return Ok(false);
        };
        match self.client.merge_queue(&page) {
            Ok(effects) => {
                self.apply_effects(link, effects)?;
            }
            Err(ClientError::StaleQueue) => {
                if self
                    .client
                    .session(&self.session)
                    .is_some_and(|replica| replica.sequence() < page.seq)
                {
                    self.queue.reads.future = Some((read, page));
                    return Ok(false);
                }
                self.queue.pages.revision = None;
                return Ok(true);
            }
            Err(error) => return Err(error.into()),
        }
        if !page.queue.known || page.queue.reading {
            return Ok(true);
        }
        if self.queue.pages.revision.as_ref() != Some(&page.queue.revision) {
            self.queue.pages.reset(
                page.queue.revision.clone(),
                self.view.codex_queue.selected.is_some(),
            );
        }
        if let Err(error) =
            self.queue
                .pages
                .accept(page, read.start, &mut self.view.codex_queue.selected)
        {
            self.queue.reads.failure = Some(ReadFailure {
                revision: self.queue.pages.revision.clone(),
                message: error.to_string(),
            });
        }
        Ok(true)
    }

    /// Expire the device-only Steer acknowledgement even while the socket is disconnected.
    pub(super) fn expire_queue_mode_notice(&mut self) -> bool {
        if self
            .view
            .viewport
            .steer_notice_until
            .is_some_and(|until| until <= Instant::now())
        {
            self.view.viewport.steer_notice_until = None;
            return true;
        }
        false
    }
}
