//! Serve independently paged queue metadata and revision-checked plain text chunks.

use carlos_proto::commands::RpcError;
use carlos_proto::ids::{QueueEntryId, QueueRevision};

use super::owner::{failure, Owner};
use super::{EngineCommand, EngineReply};
use crate::backend::BackendKind;
use crate::session::codex::queue;
use crate::session::codex::queue_state::editable_text;

impl Owner {
    pub(super) fn read_queue(&mut self, command: EngineCommand) -> Result<EngineReply, RpcError> {
        self.require_queue_backend()?;
        match command {
            EngineCommand::QueueRead {
                expected,
                after,
                limit,
                refresh,
            } => {
                if refresh {
                    if expected.is_some() || after.is_some() {
                        return Err(failure("refresh cannot continue an old queue page"));
                    }
                    self.session.clear_codex_queue_error();
                    self.session.invalidate_codex_queue();
                }
                if let Some(expected) = &expected {
                    self.require_queue_revision(expected)?;
                }
                if after.is_some() && expected.is_none() {
                    return Err(failure("queue page needs its expected revision"));
                }
                let start = after
                    .as_ref()
                    .map(|id| self.queue_index(id).map(|index| index + 1))
                    .transpose()?
                    .unwrap_or(0);
                let scope = self.session.codex_scope();
                queue::tick(
                    self.client.as_ref(),
                    &mut self.session,
                    scope,
                    &mut Default::default(),
                    false,
                );
                self.publish();
                Ok(EngineReply::Queue(self.session.queue_page(start, limit)))
            }
            EngineCommand::QueueText {
                expected,
                entry,
                offset,
                limit,
            } => self.read_queue_text(expected, entry, offset, limit),
            _ => unreachable!("typed queue read"),
        }
    }

    fn read_queue_text(
        &self,
        expected: QueueRevision,
        entry: QueueEntryId,
        offset: carlos_proto::bytes::ByteOffset,
        limit: usize,
    ) -> Result<EngineReply, RpcError> {
        self.require_queue_revision(&expected)?;
        let (_, native) = self
            .session
            .queue_entry(&entry)
            .ok_or_else(|| failure("retired or foreign queue entry"))?;
        let text = editable_text(native).ok_or_else(|| {
            failure("queue entry has attachments, spans or unsupported input; edit unavailable")
        })?;
        let start = usize::try_from(offset.get())
            .ok()
            .filter(|offset| *offset <= text.len())
            .ok_or_else(|| failure("queue text offset is beyond the accepted input"))?;
        let start = floor_utf8(text, start);
        let end = floor_utf8(
            text,
            start.saturating_add(limit.clamp(4, 65536)).min(text.len()),
        );
        Ok(EngineReply::QueueText(carlos_proto::queue::QueueText {
            session: self.session.session_id().clone(),
            revision: expected,
            entry,
            offset: carlos_proto::bytes::ByteOffset::new(start as u64),
            next_offset: carlos_proto::bytes::ByteOffset::new(end as u64),
            complete: end == text.len(),
            text: text[start..end].into(),
        }))
    }

    pub(super) fn require_queue_backend(&self) -> Result<(), RpcError> {
        if self.client.kind() != BackendKind::Codex || !self.session.codex_scope().enabled() {
            return Err(failure("this backend does not support native Codex queues"));
        }
        Ok(())
    }

    pub(super) fn require_queue_revision(&self, expected: &QueueRevision) -> Result<(), RpcError> {
        if !self.session.codex_queue.known || expected != &self.session.queue_revision() {
            return Err(failure(
                "queue changed or is unknown; refresh before submitting this draft",
            ));
        }
        Ok(())
    }

    pub(super) fn queue_index(&self, id: &QueueEntryId) -> Result<usize, RpcError> {
        self.session
            .queue_entry(id)
            .map(|(index, _)| index)
            .ok_or_else(|| failure("retired or foreign queue entry"))
    }
}

fn floor_utf8(text: &str, mut offset: usize) -> usize {
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}
