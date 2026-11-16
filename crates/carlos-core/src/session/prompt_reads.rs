//! Borrow immutable human and explicitly unknown prompt records for independent device cursors.

use carlos_proto::bytes::{ByteLength, ByteOffset};
use carlos_proto::commands::RpcError;
use carlos_proto::ids::EntryId as WireEntryId;
use carlos_proto::prompts::{
    PromptDirection, PromptRecord, PromptSelection, PromptSource, PromptText,
};

use super::entry_id::EntryId;
use super::state::SessionState;

struct Record<'a> {
    id: EntryId,
    entry: Option<EntryId>,
    text: &'a str,
}

/// Read immutable prompt facts through the owner's existing bounded read mailbox.
pub enum PromptCommand {
    Select {
        source: PromptSource,
        anchor: Option<WireEntryId>,
        direction: PromptDirection,
    },
    Text {
        prompt: WireEntryId,
        offset: ByteOffset,
        limit: usize,
    },
}

impl SessionState {
    /// Resolve one cursor against the current owner records without retaining a device selection.
    pub fn select_prompt(
        &self,
        source: PromptSource,
        anchor: Option<&WireEntryId>,
        direction: PromptDirection,
    ) -> Result<PromptSelection, RpcError> {
        let count = self.prompt_count(source);
        let index = match (anchor, direction) {
            (None, PromptDirection::Latest) => count.checked_sub(1),
            (Some(anchor), PromptDirection::Older | PromptDirection::Newer) => {
                let old = self.prompt_position(source, EntryId::from_wire(anchor))?;
                match direction {
                    PromptDirection::Older => Some(old.saturating_sub(1)),
                    PromptDirection::Newer => (old + 1 < count).then_some(old + 1),
                    PromptDirection::Latest => unreachable!("relative prompt direction"),
                }
            }
            _ => return Err(failure("prompt direction requires its original anchor")),
        };
        let record = index.and_then(|index| self.prompt_record(source, index));
        Ok(PromptSelection {
            session: self.session_id().clone(),
            source,
            record: record.map(|record| PromptRecord {
                id: record.id.to_wire(),
                entry: record.entry.map(EntryId::to_wire),
                source,
                bytes: ByteLength::new(record.text.len() as u64),
            }),
            has_older: index
                .is_some_and(|index| index > 0 || self.prompt_has_native_history(source)),
            has_newer: index.is_some_and(|index| index + 1 < count),
        })
    }

    /// Keep human/unknown recall independent from transcript paging and other device cursors.
    pub(crate) fn prompt_needs_history(&self, command: &PromptCommand) -> Result<bool, RpcError> {
        let PromptCommand::Select {
            source,
            anchor,
            direction,
        } = command
        else {
            return Ok(false);
        };
        let selected = self.select_prompt(*source, anchor.as_ref(), *direction)?;
        let stalled = match direction {
            PromptDirection::Latest => selected.record.is_none(),
            PromptDirection::Older => {
                selected.record.as_ref().map(|record| &record.id) == anchor.as_ref()
            }
            PromptDirection::Newer => false,
        };
        Ok(stalled && self.prompt_has_native_history(*source))
    }

    fn prompt_has_native_history(&self, source: PromptSource) -> bool {
        match source {
            PromptSource::Human => self.has_older_native_history(),
            PromptSource::Unknown => self.has_older_claude_history(),
        }
    }

    /// Copy only the requested piece; a retired prompt cannot select a replacement with equal text.
    pub fn prompt_text(
        &self,
        prompt: &WireEntryId,
        offset: ByteOffset,
        limit: usize,
    ) -> Result<PromptText, RpcError> {
        let id = EntryId::from_wire(prompt);
        let record = [PromptSource::Human, PromptSource::Unknown]
            .into_iter()
            .find_map(|source| {
                self.prompt_position(source, id)
                    .ok()
                    .and_then(|index| self.prompt_record(source, index))
            })
            .ok_or_else(|| failure("prompt was retired or belongs to another session"))?;
        let start =
            usize::try_from(offset.get()).map_err(|_| failure("prompt offset is too large"))?;
        if !record.text.is_char_boundary(start) {
            return Err(failure("prompt offset is outside its UTF-8 text"));
        }
        let mut end = start
            .saturating_add(limit.clamp(4, 65536))
            .min(record.text.len());
        while !record.text.is_char_boundary(end) {
            end -= 1;
        }
        Ok(PromptText {
            session: self.session_id().clone(),
            prompt: prompt.clone(),
            offset,
            total: ByteLength::new(record.text.len() as u64),
            text: record.text[start..end].to_owned(),
            next: (end < record.text.len()).then(|| ByteOffset::new(end as u64)),
        })
    }

    fn prompt_count(&self, source: PromptSource) -> usize {
        match source {
            PromptSource::Human => self.input_history().history_len(),
            PromptSource::Unknown => self.claude_history().unknown().len(),
        }
    }

    fn prompt_record(&self, source: PromptSource, index: usize) -> Option<Record<'_>> {
        match source {
            PromptSource::Human => Some(Record {
                id: self.input_history().prompt_id(index)?,
                entry: self.input_history().entry_id(index),
                text: self.input_history().text(index),
            }),
            PromptSource::Unknown => {
                let prompt = self.claude_history().unknown().get(index)?;
                Some(Record {
                    id: prompt.entry_id(),
                    entry: Some(prompt.entry_id()),
                    text: prompt.text(),
                })
            }
        }
    }

    fn prompt_position(&self, source: PromptSource, id: EntryId) -> Result<usize, RpcError> {
        (0..self.prompt_count(source))
            .find(|index| {
                self.prompt_record(source, *index)
                    .is_some_and(|record| record.id == id)
            })
            .ok_or_else(|| failure("prompt was retired or belongs to another source/session"))
    }
}

pub(super) fn failure(message: &str) -> RpcError {
    RpcError {
        code: -32012,
        message: message.into(),
    }
}

#[cfg(test)]
#[path = "prompt_read_tests.rs"]
mod tests;
