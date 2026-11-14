//! Keep the original and parsed body in one formatter thread; return only checked viewport rows.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::Arc;

use carlos_proto::source::Source;

use super::frame::{FormattedBlock, View};
use super::{Job, Reply, Request, TransferError, Wanted};
use crate::app::transcript_body::accepted_source::SourcePiece;

enum Content {
    Bytes(Arc<String>),
    Block(FormattedBlock),
}

struct Active {
    job: Job,
    content: Option<Content>,
    source: Option<Source>,
    view: Option<(u64, View)>,
    built: Option<View>,
    sent: Option<(u64, u64)>,
}

pub(super) fn run(requests: Receiver<Request>, replies: SyncSender<Reply>, wanted: Arc<Wanted>) {
    let mut active = None;
    while let Ok(request) = requests.recv() {
        if active
            .as_ref()
            .is_some_and(|active: &Active| active.job.id.0 != wanted.job.load(Ordering::Acquire))
        {
            active = None;
        }
        if let Err(error) = apply(&mut active, request, &replies, &wanted) {
            let Some(failed) = active.take() else {
                continue;
            };
            if replies
                .send(Reply::Failed {
                    job: failed.job.id,
                    error,
                })
                .is_err()
            {
                break;
            }
        }
        if let Some(current) = &mut active {
            if wanted.job.load(Ordering::Acquire) != current.job.id.0 {
                continue;
            }
            match current.render(&wanted) {
                Ok(Some(reply)) => {
                    if replies.send(reply).is_err() {
                        break;
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    let failed = active.take().expect("current job");
                    if replies
                        .send(Reply::Failed {
                            job: failed.job.id,
                            error,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
    }
}

fn apply(
    active: &mut Option<Active>,
    request: Request,
    replies: &SyncSender<Reply>,
    wanted: &Wanted,
) -> Result<(), TransferError> {
    match request {
        Request::Open(job) if wanted.job.load(Ordering::Acquire) == job.id.0 => {
            *active = Some(Active {
                job,
                content: Some(Content::Bytes(Arc::new(String::new()))),
                source: None,
                view: None,
                built: None,
                sent: None,
            });
        }
        Request::Data(id, piece) if wanted.job.load(Ordering::Acquire) == id.0 => {
            if let Some(active) = active.as_mut().filter(|active| active.job.id == id) {
                active.append(piece)?;
            }
        }
        Request::View(id, serial, view) if wanted.job.load(Ordering::Acquire) == id.0 => {
            if let Some(active) = active.as_mut().filter(|active| active.job.id == id) {
                active.view = Some((serial, view));
            }
        }
        Request::Retire(id) => {
            if active.as_ref().is_some_and(|active| active.job.id == id) {
                *active = None;
            }
        }
        #[cfg(test)]
        Request::Hold(entered, release) => {
            let _ = entered.send(());
            let _ = release.recv();
        }
        #[cfg(test)]
        Request::Barrier(done) => {
            let _ = done.send(());
        }
        #[cfg(test)]
        Request::Digest(id) => {
            if let Some(active) = active.as_ref().filter(|active| active.job.id == id) {
                let digest = blake3::hash(active.original().as_bytes());
                replies
                    .send(Reply::Digest { job: id, digest })
                    .map_err(|_| TransferError::Closed)?;
            }
        }
        _ => {}
    }
    Ok(())
}

impl Active {
    fn original(&self) -> &str {
        match self.content.as_ref().expect("owned original") {
            Content::Bytes(bytes) => bytes,
            Content::Block(block) => block.original(),
        }
    }

    fn take_original(&mut self) -> Arc<String> {
        match self.content.take().expect("owned original") {
            Content::Bytes(bytes) => bytes,
            Content::Block(block) => block.into_original(),
        }
    }

    fn append(&mut self, piece: SourcePiece) -> Result<(), TransferError> {
        let end = piece
            .offset
            .get()
            .checked_add(piece.text.len() as u64)
            .ok_or(TransferError::InvalidOffset)?;
        if piece.token.session() != self.job.token.session()
            || piece.token.entry() != self.job.token.entry()
            || piece.source.revision != self.job.revision
            || piece.offset.get() != self.original().len() as u64
            || end > piece.source.bytes.get()
            || piece.done && end != piece.source.bytes.get()
        {
            return Err(TransferError::InvalidOffset);
        }
        let mut original = self.take_original();
        let text = Arc::make_mut(&mut original);
        text.try_reserve(piece.text.len())
            .map_err(|_| TransferError::Allocation)?;
        text.push_str(&piece.text);
        self.content = Some(Content::Bytes(original));
        self.source = Some(piece.source);
        self.built = None;
        self.sent = None;
        Ok(())
    }

    fn render(&mut self, wanted: &Wanted) -> Result<Option<Reply>, TransferError> {
        let Some(source) = self.source.clone() else {
            return Ok(None);
        };
        let Some((id, view)) = self.view.clone() else {
            return Ok(None);
        };
        if wanted.view.load(Ordering::Acquire) != id
            || source.bytes.get() != self.original().len() as u64
            || self.sent == Some((id, source.bytes.get()))
        {
            return Ok(None);
        }
        if self
            .built
            .as_ref()
            .is_none_or(|old| !same_layout(old, &view))
        {
            let original = self.take_original();
            let block = FormattedBlock::new(original, &view).map_err(|_| TransferError::Format)?;
            self.content = Some(Content::Block(block));
            self.built = Some(view.clone());
        }
        let Some(Content::Block(block)) = &self.content else {
            unreachable!("prepared body")
        };
        let prepared = block
            .window(view.first, view.length)
            .map_err(|_| TransferError::Format)?;
        self.sent = Some((id, source.bytes.get()));
        Ok(Some(Reply::Frame {
            job: self.job.id,
            view: id,
            source,
            prepared,
        }))
    }
}

fn same_layout(left: &View, right: &View) -> bool {
    left.facts == right.facts && left.format == right.format && left.previous == right.previous
}
