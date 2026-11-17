//! Check original prompt addresses and exact byte intervals before a device changes its draft.

use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::Command;
use carlos_proto::ids::{EntryId, SessionId};
use carlos_proto::prompts::{PromptSelection, PromptSource, PromptText};
use serde_json::Value;

use crate::client_replies::parse;
use crate::pending::Kind;
use crate::{ClientError, ReadReply};

#[derive(Clone)]
pub(crate) enum PromptTarget {
    Select {
        session: SessionId,
        source: PromptSource,
        entry: Option<EntryId>,
    },
    Text {
        session: SessionId,
        prompt: EntryId,
        offset: ByteOffset,
        limit: usize,
    },
}

pub(crate) fn kind(command: &Command) -> Kind {
    Kind::Prompts(match command {
        Command::PromptSelect {
            session,
            source,
            anchor,
            direction,
        } => PromptTarget::Select {
            session: session.clone(),
            source: *source,
            entry: (*direction == carlos_proto::prompts::PromptDirection::AtEntry)
                .then(|| anchor.clone())
                .flatten(),
        },
        Command::PromptText {
            session,
            prompt,
            offset,
            limit,
        } => PromptTarget::Text {
            session: session.clone(),
            prompt: prompt.clone(),
            offset: *offset,
            limit: limit.unwrap_or(65536).clamp(4, 65536) as usize,
        },
        _ => unreachable!("typed prompt read"),
    })
}

pub(crate) fn reply(target: PromptTarget, value: Value) -> Result<ReadReply, ClientError> {
    match target {
        PromptTarget::Select {
            session,
            source,
            entry,
        } => {
            let selection: PromptSelection = parse(value)?;
            if selection.session != session {
                return Err(ClientError::ForeignSession);
            }
            if selection.source != source
                || entry.as_ref().is_some_and(|expected| {
                    selection
                        .record
                        .as_ref()
                        .and_then(|record| record.entry.as_ref())
                        != Some(expected)
                })
                || selection
                    .record
                    .as_ref()
                    .is_some_and(|record| record.source != source)
                || selection.record.is_none() && (selection.has_older || selection.has_newer)
            {
                return Err(invalid(
                    "prompt selection has inconsistent source or bounds",
                ));
            }
            Ok(ReadReply::PromptSelection(selection))
        }
        PromptTarget::Text {
            session,
            prompt,
            offset,
            limit,
        } => {
            let chunk: PromptText = parse(value)?;
            if chunk.session != session {
                return Err(ClientError::ForeignSession);
            }
            validate_text(&chunk, &prompt, offset, limit)?;
            Ok(ReadReply::PromptText(chunk))
        }
    }
}

fn validate_text(
    chunk: &PromptText,
    prompt: &EntryId,
    offset: ByteOffset,
    limit: usize,
) -> Result<(), ClientError> {
    let end = offset.get().checked_add(chunk.text.len() as u64);
    let next = end
        .filter(|end| *end < chunk.total.get())
        .map(ByteOffset::new);
    if &chunk.prompt != prompt
        || chunk.offset != offset
        || chunk.text.len() > limit
        || end.is_none_or(|end| end > chunk.total.get())
        || chunk.next != next
        || chunk.text.is_empty() && chunk.next.is_some()
    {
        return Err(invalid(
            "prompt text does not match its original byte interval",
        ));
    }
    Ok(())
}

fn invalid(message: &str) -> ClientError {
    ClientError::InvalidPayload(message.into())
}
