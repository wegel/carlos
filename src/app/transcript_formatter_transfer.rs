//! Admit bounded source pieces and small control requests to the source-owning worker.

use std::sync::mpsc::{SyncSender, TrySendError};

use carlos_proto::bytes::ByteOffset;

use super::super::accepted_source::SourcePieceError;
use super::{Formatter, Request, TransferError};

impl Formatter {
    pub(super) fn pump(&mut self) -> Result<bool, TransferError> {
        if !self.pump_source()? {
            return Ok(false);
        }
        let producer = self.producer.as_mut().ok_or(TransferError::Retired)?;
        if let Some((id, source)) = &producer.copy {
            if producer.offset.get() >= source.bytes.get() {
                if !send(
                    &self.sender,
                    Request::Copy(producer.job.id, *id, source.clone()),
                )? {
                    return Ok(false);
                }
                producer.copy = None;
            }
        }
        Ok(true)
    }

    fn pump_source(&mut self) -> Result<bool, TransferError> {
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

impl TransferError {
    pub(super) fn from_source(error: SourcePieceError) -> Self {
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

pub(super) fn send(sender: &SyncSender<Request>, request: Request) -> Result<bool, TransferError> {
    match sender.try_send(request) {
        Ok(()) => Ok(true),
        Err(TrySendError::Full(_)) => Ok(false),
        Err(TrySendError::Disconnected(_)) => Err(TransferError::Closed),
    }
}
