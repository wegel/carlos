//! Retain one device's queue page reader without owning UI focus, editors or native work.

use carlos_proto::commands::RequestId;

use super::queue_input::InputState;
use super::queue_reads::PageReads;
use super::QueuePageWindow;

/// Readonly queue previews and read status for one device; session facts stay in Replica.
#[derive(Default)]
pub struct QueuePanel {
    pub(crate) pages: QueuePageWindow,
    pub(crate) selection: Option<String>,
    pub(crate) refresh: bool,
    pub(crate) reads: PageReads,
    pub(crate) dirty: bool,
    pub(crate) input: InputState,
    bytes: usize,
}

impl QueuePanel {
    /// Borrow the single retained page and its global target position.
    pub fn page(&self) -> &QueuePageWindow {
        &self.pages
    }

    /// Borrow the requested opaque identity, even when it has left the latest list.
    pub fn selection(&self) -> Option<&str> {
        self.selection.as_deref()
    }

    /// Distinguish a pending page or explicit refresh from settled previews.
    pub fn is_reading(&self) -> bool {
        self.refresh || self.reads.busy()
    }

    /// Borrow this reader's failure without replacing an independent editor or mutation error.
    pub fn read_error(&self) -> Option<&str> {
        self.reads
            .failure
            .as_ref()
            .map(|failure| failure.message.as_str())
    }

    /// Report an unfinished full-text read without exposing its partial text to an editor.
    pub fn is_reading_input(&self) -> bool {
        self.input.read.is_some()
    }

    /// Borrow a full-text failure independently from page, mutation and private-editor errors.
    pub fn input_error(&self) -> Option<&str> {
        self.input.error.as_deref()
    }

    pub(crate) fn recount(&mut self) {
        self.bytes = self.pages.retained_bytes()
            + self.reads.retained_bytes()
            + self.selection.as_ref().map_or(0, String::len)
            + self.input.retained_bytes()
            + 256;
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.bytes
    }

    pub(crate) fn cancel_reads(&mut self) -> Vec<RequestId> {
        let mut ids: Vec<_> = self
            .reads
            .request
            .take()
            .map(|read| read.id)
            .into_iter()
            .collect();
        self.reads = PageReads::default();
        ids.extend(self.input.read.take().and_then(|read| read.request));
        self.input = Default::default();
        self.recount();
        ids
    }

    pub(crate) fn disconnected(&mut self) {
        self.refresh |= self.reads.refreshing();
        self.reads.request = None;
        self.reads.future = None;
        self.input = Default::default();
        self.dirty = true;
        self.recount();
    }
}
