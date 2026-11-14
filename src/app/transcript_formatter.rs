//! Send bounded accepted pieces to one source-owning formatter without blocking terminal input.

#[path = "transcript_formatter_body.rs"]
mod body;

#[path = "transcript_formatter_frame.rs"]
mod frame;

#[path = "transcript_formatter_owner.rs"]
mod owner;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;

use carlos_client::{Bodies, BodyToken};
use carlos_proto::bytes::ByteOffset;
use carlos_proto::output::OutputRevision;
use carlos_proto::source::Source;

use self::frame::{Facts, View};
use super::accepted_source::{SourcePiece, SourcePieceError};
use crate::app::flow_transcript::EntryContext;
use crate::app::render_entry_cache::{PreparedRow, RowFormat};
use crate::app::transcript_input::RenderInput;

/// A formatter job outlives socket reconnects but never a replaced source or cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct JobId(u64);

#[derive(Clone)]
struct Job {
    id: JobId,
    token: BodyToken,
    revision: OutputRevision,
}

/// Distinguish backpressure from invalid or retired accepted source pieces.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum TransferError {
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
pub(super) enum Transfer {
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
    #[cfg(test)]
    Barrier(std::sync::mpsc::Sender<()>),
    #[cfg(test)]
    Hold(std::sync::mpsc::Sender<()>, Receiver<()>),
    #[cfg(test)]
    Digest(JobId),
}

/// Return rows and identity alone; the original string stays in the worker.
pub(super) enum Reply {
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
}

struct Producer {
    job: Job,
    offset: ByteOffset,
    opening: bool,
    pending: Option<SourcePiece>,
    view: Option<(u64, View)>,
    view_sent: bool,
    known: bool,
}

/// Bound queued bytes to four64KiB pieces and one unsent piece outside the pure cache.
pub(super) struct Formatter {
    sender: SyncSender<Request>,
    replies: Receiver<Reply>,
    wanted: Arc<Wanted>,
    producer: Option<Producer>,
    next_job: u64,
    next_view: u64,
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
        });
        Ok(id)
    }

    /// Coalesce the latest viewport request without copying its complete source.
    pub fn show(
        &mut self,
        facts: RenderInput<'_>,
        format: RowFormat,
        previous: Option<EntryContext>,
        first: usize,
        length: usize,
    ) -> Result<u64, TransferError> {
        let producer = self.producer.as_mut().ok_or(TransferError::Retired)?;
        let view = View {
            facts: Facts::from_input(facts),
            format,
            previous,
            first,
            length,
        };
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
        let piece = SourcePiece::from_cache(
            cache,
            &producer.job.token,
            producer.job.revision,
            producer.offset,
        )
        .map_err(TransferError::from_source)?;
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
            let Some(producer) = &self.producer else {
                continue;
            };
            let accepted = match &reply {
                Reply::Frame { job, view, .. } => {
                    *job == producer.job.id
                        && producer.view.as_ref().is_some_and(|(id, _)| id == view)
                }
                Reply::Failed { job, .. } => *job == producer.job.id,
                #[cfg(test)]
                Reply::Digest { job, .. } => *job == producer.job.id,
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

    fn pump(&mut self) -> Result<bool, TransferError> {
        let producer = self.producer.as_mut().ok_or(TransferError::Retired)?;
        if producer.opening {
            if !send(&self.sender, Request::Open(producer.job.clone()))? {
                return Ok(false);
            }
            producer.opening = false;
        }
        if !producer.view_sent {
            if let Some((id, view)) = &producer.view {
                if !send(
                    &self.sender,
                    Request::View(producer.job.id, *id, view.clone()),
                )? {
                    return Ok(false);
                }
                producer.view_sent = true;
            }
        }
        let Some(piece) = producer.pending.take() else {
            return Ok(true);
        };
        let end = piece
            .offset
            .get()
            .checked_add(piece.text.len() as u64)
            .ok_or(TransferError::InvalidOffset)?;
        match self.sender.try_send(Request::Data(producer.job.id, piece)) {
            Ok(()) => {
                producer.offset = ByteOffset::new(end);
                producer.known = true;
                Ok(true)
            }
            Err(TrySendError::Full(Request::Data(_, piece))) => {
                producer.pending = Some(piece);
                Ok(false)
            }
            Err(TrySendError::Disconnected(_)) => Err(TransferError::Closed),
            Err(TrySendError::Full(_)) => {
                unreachable!("the failed request is the admitted source piece")
            }
        }
    }
}

impl Drop for Formatter {
    fn drop(&mut self) {
        self.wanted.job.store(0, Ordering::Release);
    }
}

impl TransferError {
    fn from_source(error: SourcePieceError) -> Self {
        match error {
            SourcePieceError::Retired => Self::Retired,
            SourcePieceError::Unknown => Self::Unknown,
            SourcePieceError::Replaced => Self::Replaced,
            SourcePieceError::MissingPrefix {
                requested,
                available,
            } => Self::MissingPrefix {
                requested,
                available,
            },
            SourcePieceError::InvalidOffset => Self::InvalidOffset,
            SourcePieceError::Allocation => Self::Allocation,
        }
    }
}

fn send(sender: &SyncSender<Request>, request: Request) -> Result<bool, TransferError> {
    match sender.try_send(request) {
        Ok(()) => Ok(true),
        Err(TrySendError::Full(_)) => Ok(false),
        Err(TrySendError::Disconnected(_)) => Err(TransferError::Closed),
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
