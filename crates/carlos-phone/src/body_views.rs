//! Read bounded original byte intervals and classified tool windows across the native boundary.

use std::sync::Arc;

use carlos_client::ClientError;
use carlos_proto::session::OutputKind;

use crate::{PhoneBody, PhoneClient, PhoneError};

/// Byte addresses describe the retained source interval without copying its entire prefix.
#[derive(uniffi::Record)]
pub struct PhoneSourceWindow {
    pub start: u64,
    pub end: u64,
    pub total_bytes: Option<u64>,
    pub has_more: bool,
    pub done: bool,
}

/// Actual normalized piece classes; Swift chooses the matching text color.
pub type PhoneOutputKind = OutputKind;
#[uniffi::remote(Enum)]
enum PhoneOutputKind {
    Plain,
    Command,
    Success,
    Error,
    Addition,
    Deletion,
    Hunk,
    Context,
}

/// One normalized piece retains its exact newlines and never masquerades as original copy text.
#[derive(uniffi::Record)]
pub struct PhoneOutputPiece {
    pub kind: PhoneOutputKind,
    pub text: String,
}

/// A bounded display window reports an omitted prefix explicitly.
#[derive(uniffi::Record)]
pub struct PhoneOutputWindow {
    pub pieces: Vec<PhoneOutputPiece>,
    pub missing_prefix: bool,
    pub has_more: bool,
    pub done: bool,
}

#[uniffi::export]
impl PhoneClient {
    /// Inspect progress before asking for at most one bounded slice of original text.
    pub fn source_window(&self, body: Arc<PhoneBody>) -> Result<PhoneSourceWindow, PhoneError> {
        body.require_owner(self)?;
        let state = self.lock()?;
        let window = state
            .client
            .bodies()
            .source(&body.token)
            .ok_or(ClientError::RetiredBody)?;
        Ok(PhoneSourceWindow {
            start: window.start().get(),
            end: window.end().get(),
            total_bytes: window.descriptor().map(|s| s.bytes.get()),
            has_more: window.has_more(),
            done: window.is_done(),
        })
    }

    /// Copy at most 64KiB from an accepted UTF-8 boundary, without duplicating a growing document.
    pub fn source_text(
        &self,
        body: Arc<PhoneBody>,
        offset: u64,
        limit: u32,
    ) -> Result<String, PhoneError> {
        body.require_owner(self)?;
        if limit == 0 || limit > 65536 {
            return Err(PhoneError::InvalidInput {
                message: "Read one to 65536 source bytes".into(),
            });
        }
        let state = self.lock()?;
        let window = state
            .client
            .bodies()
            .source(&body.token)
            .ok_or(ClientError::RetiredBody)?;
        if offset < window.start().get() || offset > window.end().get() {
            return Err(PhoneError::StaleRead {
                message: "Requested source bytes are outside the retained window".into(),
            });
        }
        slice(
            window.pieces(),
            offset - window.start().get(),
            limit as usize,
        )
    }

    /// Read only this expanded tool's bounded normalized window, including its true prefix state.
    pub fn output_window(&self, body: Arc<PhoneBody>) -> Result<PhoneOutputWindow, PhoneError> {
        body.require_owner(self)?;
        let state = self.lock()?;
        let window = state
            .client
            .bodies()
            .output(&body.token)
            .ok_or(ClientError::RetiredBody)?;
        Ok(PhoneOutputWindow {
            pieces: window
                .pieces()
                .map(|p| PhoneOutputPiece {
                    kind: p.kind,
                    text: p.text.clone(),
                })
                .collect(),
            missing_prefix: window.has_missing_prefix(),
            has_more: window.has_more(),
            done: window.is_done(),
        })
    }
}

fn slice<'a>(
    pieces: impl Iterator<Item = &'a str>,
    mut skip: u64,
    limit: usize,
) -> Result<String, PhoneError> {
    let mut text = String::new();
    for piece in pieces {
        if skip >= piece.len() as u64 {
            skip -= piece.len() as u64;
            continue;
        }
        let suffix = piece
            .get(skip as usize..)
            .ok_or_else(|| PhoneError::InvalidInput {
                message: "Source reads must begin at a UTF-8 boundary".into(),
            })?;
        skip = 0;
        let mut take = suffix.len().min(limit - text.len());
        while !suffix.is_char_boundary(take) {
            take -= 1;
        }
        text.push_str(&suffix[..take]);
        if take < suffix.len() || text.len() == limit {
            break;
        }
    }
    Ok(text)
}
