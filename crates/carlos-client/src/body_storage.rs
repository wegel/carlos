//! Apply one body chunk under the shared budget and opt into one contiguous source buffer.

use carlos_proto::ids::{EntryId, SessionId};
use carlos_proto::output::{OutputChunk, OutputRevision};
use carlos_proto::source::SourceChunk;

use super::{Bodies, Body, BodyToken, Key, BODY_BYTES};
use crate::ClientError;

impl Bodies {
    /// Retain original source contiguously within the shared eight-MiB body-data allowance.
    pub fn open_source_document(
        &mut self,
        session: SessionId,
        entry: EntryId,
        revision: OutputRevision,
    ) -> Result<BodyToken, ClientError> {
        let mut token = self.open_source(session, entry, revision)?;
        token.document = true;
        self.windows
            .get_mut(&token.key)
            .expect("new document window")
            .document = true;
        Ok(token)
    }

    /// Admit one exact source reply only into its still-current read lifetime and cursor.
    pub fn apply_source(
        &mut self,
        token: &BodyToken,
        chunk: SourceChunk,
    ) -> Result<bool, ClientError> {
        if token.key.entry != chunk.entry {
            return Err(ClientError::InvalidBody);
        }
        let available = self.available(token)?;
        let window = self.window_mut(token).ok_or(ClientError::RetiredBody)?;
        let Body::Source(window) = &mut window.body else {
            return Err(ClientError::InvalidBody);
        };
        window.apply(chunk, available, token.document)
    }

    /// Admit normalized pieces without sharing source cursors or replaying a duplicated chunk.
    pub fn apply_output(
        &mut self,
        token: &BodyToken,
        chunk: OutputChunk,
    ) -> Result<bool, ClientError> {
        if token.key.entry != chunk.entry {
            return Err(ClientError::InvalidBody);
        }
        let available = self.available(token)?;
        let window = self.window_mut(token).ok_or(ClientError::RetiredBody)?;
        let Body::Output(window) = &mut window.body else {
            return Err(ClientError::InvalidBody);
        };
        window.apply(chunk, available)
    }

    fn available(&self, token: &BodyToken) -> Result<usize, ClientError> {
        self.window(token).ok_or(ClientError::RetiredBody)?;
        Ok(BODY_BYTES.saturating_sub(self.retained_except(&token.key)))
    }

    fn retained_except(&self, except: &Key) -> usize {
        self.windows
            .iter()
            .filter(|(key, _)| *key != except)
            .map(|(_, window)| match &window.body {
                Body::Source(window) => window.pieces.bytes,
                Body::Output(window) => window.pieces.bytes,
            })
            .sum()
    }
}
