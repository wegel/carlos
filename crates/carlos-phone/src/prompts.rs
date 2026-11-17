//! Resolve an exact old user row before exposing its original text or submitting a branch.

use carlos_client::{ReadReply, ReadResult};
use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::{Command, RequestId, SendPolicy};
use carlos_proto::ids::EntryId;
use carlos_proto::prompts::{PromptDirection, PromptSource};
use carlos_proto::session::EntryKind;

use crate::session_calls::session_id;
use crate::{PhoneClient, PhoneError, PhoneFrame, PhoneSubmission};

/// The phone captures this choice with each send, independently of later UI changes.
pub type PhoneSendMode = SendPolicy;
#[uniffi::remote(Enum)]
enum PhoneSendMode {
    Steer,
    Queue,
}

/// An immutable editable prompt may have a different address from its visible transcript row.
#[derive(uniffi::Record)]
pub struct PhonePrompt {
    pub id: String,
    pub entry: Option<String>,
    pub bytes: u64,
}

/// Only the original request's editor may accept a selection or original text chunk.
#[derive(uniffi::Enum)]
pub enum PhonePromptReply {
    Selected {
        request_id: i64,
        session: String,
        prompt: Option<PhonePrompt>,
    },
    Text {
        request_id: i64,
        session: String,
        prompt: String,
        offset: u64,
        total: u64,
        text: String,
        next: Option<u64>,
    },
}

impl PhonePromptReply {
    pub(crate) fn from_read(result: ReadResult) -> Option<Self> {
        let Some(RequestId::Integer(request_id)) = result.id else {
            return None;
        };
        Some(match result.reply {
            ReadReply::PromptSelection(selection) => Self::Selected {
                request_id,
                session: selection.session.to_string(),
                prompt: selection.record.map(|record| PhonePrompt {
                    id: record.id.to_string(),
                    entry: record.entry.map(|entry| entry.to_string()),
                    bytes: record.bytes.get(),
                }),
            },
            ReadReply::PromptText(chunk) => Self::Text {
                request_id,
                session: chunk.session.to_string(),
                prompt: chunk.prompt.to_string(),
                offset: chunk.offset.get(),
                total: chunk.total.get(),
                text: chunk.text,
                next: chunk.next.map(|offset| offset.get()),
            },
            _ => return None,
        })
    }
}

#[uniffi::export]
impl PhoneClient {
    /// Resolve a saved human prompt from its exact visible row, without changing backend work.
    pub fn select_prompt_for_entry(
        &self,
        session: String,
        entry: String,
    ) -> Result<PhoneFrame, PhoneError> {
        let session = session_id(&session)?;
        let entry = entry_id(&entry)?;
        let mut state = self.lock()?;
        state.require_ready_session(&session)?;
        if state
            .client
            .session(&session)
            .and_then(|replica| replica.entry(&entry))
            .is_none_or(|entry| entry.kind != EntryKind::User)
        {
            return Err(PhoneError::StaleRead {
                message: "Choose a saved user message in this session".into(),
            });
        }
        let connection = state.require_connection()?;
        let outgoing = state.client.read(
            connection,
            Command::PromptSelect {
                session,
                source: PromptSource::Human,
                anchor: Some(entry),
                direction: PromptDirection::AtEntry,
            },
        )?;
        state.frame(outgoing)
    }

    /// Read at most 64KiB of the selected original prompt at a checked UTF-8 byte offset.
    pub fn read_prompt_text(
        &self,
        session: String,
        prompt: String,
        offset: u64,
    ) -> Result<PhoneFrame, PhoneError> {
        let session = session_id(&session)?;
        let prompt = entry_id(&prompt)?;
        let mut state = self.lock()?;
        state.require_ready_session(&session)?;
        let connection = state.require_connection()?;
        let outgoing = state.client.read(
            connection,
            Command::PromptText {
                session,
                prompt,
                offset: ByteOffset::new(offset),
                limit: Some(65536),
            },
        )?;
        state.frame(outgoing)
    }

    /// Fork before the selected immutable prompt, retaining this device's edited text on failure.
    pub fn rewind_prompt(
        &self,
        session: String,
        prompt: String,
        text: String,
    ) -> Result<PhoneSubmission, PhoneError> {
        let session = session_id(&session)?;
        let entry = entry_id(&prompt)?;
        if text.trim().is_empty() {
            return Err(PhoneError::InvalidInput {
                message: "Enter the edited message first".into(),
            });
        }
        let mut state = self.lock()?;
        state.require_ready_session(&session)?;
        state.submit(|operation| Command::Rewind {
            session,
            operation,
            entry,
            text,
        })
    }
}

fn entry_id(value: &str) -> Result<EntryId, PhoneError> {
    value.parse().map_err(|_| PhoneError::InvalidInput {
        message: "Invalid prompt address".into(),
    })
}
