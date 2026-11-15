//! Send bounded accepted pieces to one source-owning formatter without blocking terminal input.

#[path = "transcript_formatter_body.rs"]
mod body;

#[path = "transcript_formatter_frame.rs"]
mod frame;

#[path = "transcript_formatter_owner.rs"]
mod owner;

#[path = "transcript_formatter_transfer.rs"]
mod transfer;

#[path = "transcript_formatter_selection.rs"]
mod selected;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Arc;
use std::thread;

use carlos_client::{Bodies, BodyToken};
use carlos_proto::bytes::ByteOffset;
use carlos_proto::output::OutputRevision;
use carlos_proto::source::Source;

pub(in crate::app) use self::frame::{Facts, View};
pub(in crate::app) use self::selected::{SelectionSlice, SelectionTarget, Target};
use super::accepted_source::SourcePiece;
#[cfg(test)]
use crate::app::flow_transcript::EntryContext;
use crate::app::render_entry_cache::PreparedRow;
#[cfg(test)]
use crate::app::render_entry_cache::RowFormat;
#[cfg(test)]
use crate::app::transcript_input::RenderInput;

/// A formatter job outlives socket reconnects but never a replaced source or cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) struct JobId(u64);

#[derive(Clone)]
struct Job {
    id: JobId,
    token: BodyToken,
    revision: OutputRevision,
}

/// Distinguish backpressure from invalid or retired accepted source pieces.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::app) enum TransferError {
    Closed,
    Exhausted,
    Retired,
    Unknown,
    Replaced,
    MissingPrefix {
        requested: ByteOffset,
        available: ByteOffset,
    },
    InvalidOffset,
    Allocation,
    Format,
}

/// Tell the platform whether it should pause its source watch or await more accepted bytes.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::app) enum Transfer {
    Queued,
    Busy,
    Waiting,
    CaughtUp,
}

enum Request {
    Open(Job),
    Data(JobId, SourcePiece),
    View(JobId, u64, View),
    Retire(JobId),
    Copy(JobId, u64, Source),
    CancelCopy,
    SelectionRows {
        slice: SelectionSlice,
        rows: crate::app::render_entry_cache::PaintedRows,
        first: usize,
        length: usize,
    },
    #[cfg(test)]
    Barrier(std::sync::mpsc::Sender<()>),
    #[cfg(test)]
    Hold(std::sync::mpsc::Sender<()>, Receiver<()>),
    #[cfg(test)]
    Digest(JobId),
}

/// Return rows and identity alone; the original string stays in the worker.
pub(in crate::app) enum Reply {
    Frame {
        job: JobId,
        view: u64,
        source: Source,
        prepared: PreparedRow,
    },
    Failed {
        job: JobId,
        error: TransferError,
    },
    Copied {
        copy: u64,
        backend: crate::clipboard::ClipboardBackend,
    },
    Selected {
        copy: u64,
    },
    CopyFailed {
        copy: u64,
        error: TransferError,
    },
    #[cfg(test)]
    Digest {
        job: JobId,
        digest: blake3::Hash,
    },
}

#[derive(Default)]
struct Wanted {
    job: AtomicU64,
    view: AtomicU64,
    copy: AtomicU64,
}

struct Producer {
    job: Job,
    offset: ByteOffset,
    opening: bool,
    pending: Option<SourcePiece>,
    view: Option<(u64, View)>,
    view_sent: bool,
    known: bool,
    copy: Option<(u64, Source)>,
}

/// Bound queued bytes to four64KiB pieces and one unsent piece outside the pure cache.
pub(in crate::app) struct Formatter {
    sender: SyncSender<Request>,
    replies: Receiver<Reply>,
    wanted: Arc<Wanted>,
    producer: Option<Producer>,
    next_job: u64,
    next_view: u64,
    next_copy: u64,
}

impl Formatter {
    /// Spawn one platform worker; it owns no socket, backend, clock or session replica.
    pub fn new() -> std::io::Result<Self> {
        let (sender, requests) = sync_channel(4);
        let (replies, receiver) = sync_channel(1);
        let wanted = Arc::new(Wanted::default());
        let worker_wanted = Arc::clone(&wanted);
        thread::Builder::new()
            .name("carlos-format".into())
            .spawn(move || owner::run(requests, replies, worker_wanted))?;
        Ok(Self {
            sender,
            replies: receiver,
            wanted,
            producer: None,
            next_job: 0,
            next_view: 0,
            next_copy: 0,
        })
    }

    /// Retire earlier results even when a restarted cache reuses exactly the same BodyToken.
    pub fn open(
        &mut self,
        token: BodyToken,
        revision: OutputRevision,
    ) -> Result<JobId, TransferError> {
        self.next_job = self
            .next_job
            .checked_add(1)
            .ok_or(TransferError::Exhausted)?;
        let id = JobId(self.next_job);
        self.wanted.job.store(id.0, Ordering::Release);
        self.wanted.view.store(0, Ordering::Release);
        self.producer = Some(Producer {
            job: Job {
                id,
                token,
                revision,
            },
            offset: ByteOffset::new(0),
            opening: true,
            pending: None,
            view: None,
            view_sent: false,
            known: false,
            copy: None,
        });
        Ok(id)
    }

    /// Coalesce the latest viewport request without copying its complete source.
    #[cfg(test)]
    pub fn show(
        &mut self,
        facts: RenderInput<'_>,
        format: RowFormat,
        previous: Option<EntryContext>,
        first: usize,
        length: usize,
    ) -> Result<u64, TransferError> {
        let view = View {
            target: Default::default(),
            facts: Facts::from_input(facts),
            format,
            previous,
            first,
            length,
        };
        self.request_view(view)
    }

    /// Keep display and private selection requests distinct while reusing their body layout.
    pub fn request_view(&mut self, view: View) -> Result<u64, TransferError> {
        let producer = self.producer.as_mut().ok_or(TransferError::Retired)?;
        if let Target::Selection(target) = &view.target {
            if let Some(piece) = &mut producer.pending {
                let remaining = target.source.bytes.get().saturating_sub(piece.offset.get());
                if remaining == 0 {
                    producer.pending = None;
                } else if remaining < piece.text.len() as u64 {
                    let length = remaining as usize;
                    if !piece.text.is_char_boundary(length) {
                        return Err(TransferError::InvalidOffset);
                    }
                    piece.text.truncate(length);
                    piece.done = false;
                }
            }
        }
        if let Some((id, old)) = &producer.view {
            if old == &view {
                return Ok(*id);
            }
        }
        self.next_view = self
            .next_view
            .checked_add(1)
            .ok_or(TransferError::Exhausted)?;
        producer.view = Some((self.next_view, view));
        producer.view_sent = false;
        self.wanted.view.store(self.next_view, Ordering::Release);
        self.pump()?;
        Ok(self.next_view)
    }

    /// Copy from the checked cache only after earlier unsent data has entered the bounded queue.
    pub fn feed(&mut self, cache: &Bodies) -> Result<Transfer, TransferError> {
        if !self.pump()? {
            return Ok(Transfer::Busy);
        }
        let producer = self.producer.as_mut().ok_or(TransferError::Retired)?;
        let limit = producer
            .view
            .as_ref()
            .and_then(|(_, view)| match &view.target {
                Target::Selection(target) => Some(target.source.bytes.get()),
                Target::Display => None,
            });
        if limit.is_some_and(|limit| producer.offset.get() >= limit) {
            return Ok(Transfer::CaughtUp);
        }
        let mut piece = SourcePiece::from_cache(
            cache,
            &producer.job.token,
            producer.job.revision,
            producer.offset,
        )
        .map_err(TransferError::from_source)?;
        if let Some(limit) = limit {
            let length = usize::try_from(limit - producer.offset.get())
                .map_err(|_| TransferError::InvalidOffset)?;
            if length < piece.text.len() {
                if !piece.text.is_char_boundary(length) {
                    return Err(TransferError::InvalidOffset);
                }
                piece.text.truncate(length);
                piece.done = false;
            }
        }
        if piece.text.is_empty() && producer.known {
            return Ok(if producer.offset.get() == piece.source.bytes.get() {
                Transfer::CaughtUp
            } else {
                Transfer::Waiting
            });
        }
        producer.pending = Some(piece);
        Ok(if self.pump()? {
            Transfer::Queued
        } else {
            Transfer::Busy
        })
    }

    /// Drain only current source/view replies; stale worker completions cannot repaint another job.
    pub fn poll(&mut self) -> Vec<Reply> {
        let mut replies = Vec::new();
        for _ in 0..4 {
            let Ok(reply) = self.replies.try_recv() else {
                break;
            };
            let accepted = match &reply {
                Reply::Copied { copy, .. }
                | Reply::Selected { copy }
                | Reply::CopyFailed { copy, .. } => {
                    self.wanted.copy.load(Ordering::Acquire) == *copy
                }
                _ => self.producer.as_ref().is_some_and(|producer| match &reply {
                    Reply::Frame { job, view, .. } => {
                        *job == producer.job.id
                            && producer.view.as_ref().is_some_and(|(id, _)| id == view)
                    }
                    Reply::Failed { job, .. } => *job == producer.job.id,
                    #[cfg(test)]
                    Reply::Digest { job, .. } => *job == producer.job.id,
                    _ => false,
                }),
            };
            if accepted {
                replies.push(reply);
            }
        }
        if let Some(job) = self.producer.as_ref().map(|producer| producer.job.id) {
            if let Err(error) = self.pump() {
                replies.push(Reply::Failed { job, error });
                self.retire();
            }
        }
        replies
    }

    /// Forget a completed private layout without dropping its untruncated original.
    pub fn clear_view(&mut self) {
        self.wanted.view.store(0, Ordering::Release);
        if let Some(producer) = &mut self.producer {
            producer.view = None;
            producer.view_sent = false;
        }
    }

    /// Locate the last piece admitted to the worker before an explicit source seek.
    pub fn offset(&self) -> Option<ByteOffset> {
        self.producer.as_ref().map(|producer| producer.offset)
    }

    /// Release a collapsed or replaced original without accepting its late worker results.
    pub fn retire(&mut self) {
        self.wanted.job.store(0, Ordering::Release);
        if let Some(producer) = self.producer.take() {
            let _ = self.sender.try_send(Request::Retire(producer.job.id));
        }
    }
}

impl Drop for Formatter {
    fn drop(&mut self) {
        self.wanted.job.store(0, Ordering::Release);
        self.wanted.copy.store(0, Ordering::Release);
    }
}

#[cfg(test)]
#[path = "transcript_formatter_body_tests.rs"]
mod body_tests;

#[cfg(test)]
#[path = "transcript_formatter_frame_tests.rs"]
mod frame_tests;

#[cfg(test)]
#[path = "transcript_formatter_owner_tests.rs"]
mod owner_tests;
