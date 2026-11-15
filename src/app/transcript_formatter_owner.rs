//! Keep the original and parsed body in one formatter thread; return only checked viewport rows.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::Arc;

use carlos_proto::source::Source;

use super::frame::{FormattedBlock, View};
use super::selected::{self, Selected, Target};
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
    let mut selected: Option<Selected> = None;
    while let Ok(request) = requests.recv() {
        if selected
            .as_ref()
            .is_some_and(|copy| copy.id != wanted.copy.load(Ordering::Acquire))
        {
            selected = None;
        }
        if active
            .as_ref()
            .is_some_and(|active: &Active| active.job.id.0 != wanted.job.load(Ordering::Acquire))
        {
            active = None;
        }
        if let Err(error) = apply(&mut active, request, &replies, &wanted, &mut selected) {
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
            match current.render(&wanted, &mut selected) {
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
    selected: &mut Option<Selected>,
) -> Result<(), TransferError> {
    match request {
        Request::Open(job) if wanted.job.load(Ordering::Acquire) == job.id.0 => {
            *active = Some(Active::new(job));
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
        Request::SelectionRows {
            slice,
            rows,
            first,
            length,
        } => {
            let reply = selected::reply(selected, wanted, &slice, &rows, first, length);
            replies.send(reply).map_err(|_| TransferError::Closed)?;
        }
        Request::Copy(id, serial, source)
            if wanted.job.load(Ordering::Acquire) == id.0
                && wanted.copy.load(Ordering::Acquire) == serial =>
        {
            copy_source(active.as_ref(), id, serial, &source, replies)?;
        }
        Request::Retire(id) if active.as_ref().is_some_and(|active| active.job.id == id) => {
            *active = None;
        }
        #[cfg(test)]
        request @ (Request::Hold(..) | Request::Barrier(..) | Request::Digest(..)) => {
            diagnostic(active.as_ref(), request, replies)?;
        }
        _ => {}
    }
    Ok(())
}

impl Active {
    fn new(job: Job) -> Self {
        Self {
            job,
            content: Some(Content::Bytes(Arc::new(String::new()))),
            source: None,
            view: None,
            built: None,
            sent: None,
        }
    }

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
        let updated = match (self.content.as_mut().expect("owned source"), &self.built) {
            (Content::Block(block), Some(view)) => block.append(&piece.text, view).unwrap_or(false),
            _ => false,
        };
        if !updated {
            let mut original = self.take_original();
            let text = Arc::make_mut(&mut original);
            if text.len() as u64 != end {
                text.try_reserve(piece.text.len())
                    .map_err(|_| TransferError::Allocation)?;
                text.push_str(&piece.text);
            }
            self.content = Some(Content::Bytes(original));
            self.built = None;
        }
        self.source = Some(piece.source);
        self.sent = None;
        Ok(())
    }

    fn select_prefix(&mut self, source: &Source) -> Result<(), TransferError> {
        if source.revision != self.job.revision {
            return Err(TransferError::Replaced);
        }
        let length =
            usize::try_from(source.bytes.get()).map_err(|_| TransferError::InvalidOffset)?;
        if self.original().len() != length {
            if !self.original().is_char_boundary(length) {
                return Err(TransferError::InvalidOffset);
            }
            let mut original = self.take_original();
            Arc::make_mut(&mut original).truncate(length);
            self.content = Some(Content::Bytes(original));
            self.built = None;
        }
        Ok(())
    }

    fn render(
        &mut self,
        wanted: &Wanted,
        selected: &mut Option<Selected>,
    ) -> Result<Option<Reply>, TransferError> {
        let Some(mut source) = self.source.clone() else {
            return Ok(None);
        };
        let Some((id, view)) = self.view.clone() else {
            return Ok(None);
        };
        if let Target::Selection(target) = &view.target {
            if wanted.copy.load(Ordering::Acquire) != target.slice.copy
                || wanted.view.load(Ordering::Acquire) != id
            {
                return Ok(None);
            }
            if target.source.revision != self.job.revision {
                return Ok(Some(Reply::CopyFailed {
                    copy: target.slice.copy,
                    error: TransferError::Replaced,
                }));
            }
            if (self.original().len() as u64) < target.source.bytes.get() {
                return Ok(None);
            }
            if let Err(error) = self.select_prefix(&target.source) {
                return Ok(Some(Reply::CopyFailed {
                    copy: target.slice.copy,
                    error,
                }));
            }
            source = target.source.clone();
        }
        if view.length == 0
            || wanted.view.load(Ordering::Acquire) != id
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
        self.sent = Some((id, source.bytes.get()));
        self.reply(id, source, &view, wanted, selected)
    }

    fn reply(
        &self,
        id: u64,
        source: Source,
        view: &View,
        wanted: &Wanted,
        selected: &mut Option<Selected>,
    ) -> Result<Option<Reply>, TransferError> {
        let Some(Content::Block(block)) = &self.content else {
            unreachable!("prepared body")
        };
        Ok(Some(match &view.target {
            Target::Display => {
                let mut prepared = block
                    .window(view.first, view.length)
                    .map_err(|_| TransferError::Format)?;
                prepared.source = Some(source.clone());
                Reply::Frame {
                    job: self.job.id,
                    view: id,
                    source,
                    prepared,
                }
            }
            Target::Selection(target) => {
                selected::block(selected, wanted, target, block, view.first, view.length)
                    .unwrap_or_else(|error| Reply::CopyFailed {
                        copy: target.slice.copy,
                        error,
                    })
            }
        }))
    }
}

fn same_layout(left: &View, right: &View) -> bool {
    left.facts == right.facts && left.format == right.format && left.previous == right.previous
}

fn copy_source(
    active: Option<&Active>,
    id: super::JobId,
    serial: u64,
    source: &Source,
    replies: &SyncSender<Reply>,
) -> Result<(), TransferError> {
    let Some(current) = active.filter(|active| active.job.id == id) else {
        return Ok(());
    };
    let end = usize::try_from(source.bytes.get()).map_err(|_| TransferError::InvalidOffset)?;
    if source.revision != current.job.revision {
        return Err(TransferError::Replaced);
    }
    let text = current
        .original()
        .get(..end)
        .ok_or(TransferError::InvalidOffset)?;
    let backend = crate::clipboard::try_copy_clipboard(text);
    replies
        .send(Reply::Copied {
            copy: serial,
            backend,
        })
        .map_err(|_| TransferError::Closed)
}

#[cfg(test)]
fn diagnostic(
    active: Option<&Active>,
    request: Request,
    replies: &SyncSender<Reply>,
) -> Result<(), TransferError> {
    match request {
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
            if let Some(active) = active.filter(|active| active.job.id == id) {
                let digest = blake3::hash(active.original().as_bytes());
                replies
                    .send(Reply::Digest { job: id, digest })
                    .map_err(|_| TransferError::Closed)?;
            }
        }
        _ => unreachable!("test diagnostic request"),
    }
    Ok(())
}
