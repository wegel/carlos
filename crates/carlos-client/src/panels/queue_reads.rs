//! Keep one owned queue request and one reply waiting for its numbered session facts.

use carlos_proto::commands::RequestId;
use carlos_proto::ids::QueueRevision;
use carlos_proto::queue::QueuePage;

pub(crate) struct PageRead {
    pub id: RequestId,
    pub start: usize,
    pub refresh: bool,
    pub revision: Option<QueueRevision>,
}

pub(crate) struct ReadFailure {
    pub revision: Option<QueueRevision>,
    pub message: String,
}

#[derive(Default)]
pub(crate) struct PageReads {
    pub request: Option<PageRead>,
    pub future: Option<(PageRead, QueuePage)>,
    pub failure: Option<ReadFailure>,
}

impl PageReads {
    pub(crate) fn busy(&self) -> bool {
        self.request.is_some() || self.future.is_some()
    }

    pub(crate) fn refreshing(&self) -> bool {
        self.request.as_ref().is_some_and(|read| read.refresh)
            || self.future.as_ref().is_some_and(|(read, _)| read.refresh)
    }

    pub(crate) fn retire_failure(&mut self, revision: &QueueRevision) {
        if self
            .failure
            .as_ref()
            .is_some_and(|failure| failure.revision.as_ref().is_some_and(|old| old != revision))
        {
            self.failure = None;
        }
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.failure
            .as_ref()
            .map_or(0, |failure| failure.message.len())
            + self.future.as_ref().map_or(0, |(_, page)| {
                page.queue.notice.len()
                    + page.queue.read_error.as_ref().map_or(0, String::len)
                    + page
                        .entries
                        .iter()
                        .map(|entry| {
                            entry.preview.len()
                                + entry.client_message.len()
                                + entry.id.as_str().len()
                                + entry.display_id.as_ref().map_or(0, String::len)
                                + 128
                        })
                        .sum::<usize>()
            })
    }
}
