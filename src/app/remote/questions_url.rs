//! Copy one captured browser URL through the shared bounded clipboard worker and permission.

use anyhow::{ensure, Result};
use carlos_proto::ids::EntryId;
use carlos_proto::session::{Approval, ApprovalInput};

use super::questions::fingerprint;
use super::Device;
use crate::app::transcript_body::formatter::TransferError;
use crate::clipboard::{clipboard_backend_label, ClipboardBackend};

pub(super) struct UrlCopy {
    card: EntryId,
    id: u64,
    fingerprint: [u8; 32],
    url: String,
    offset: usize,
    queued: bool,
}

impl Device {
    pub(super) fn question_url_tick(&mut self) -> bool {
        match self.drive_question_url() {
            Ok(changed) => changed,
            Err(error) => {
                if let Some(copy) = self.questions.url.take() {
                    if self.documents.formatting.copy_is_current(copy.id) {
                        self.documents.formatting.cancel_copy();
                    }
                    self.question_url_status(
                        &copy,
                        format!("Cannot copy the original browser URL: {error}"),
                    );
                }
                true
            }
        }
    }
    pub(super) fn copy_question_url(&mut self, card: &Approval) -> Result<()> {
        let Some(ApprovalInput::Browser { url }) = &card.input else {
            return Ok(());
        };
        ensure!(!url.is_empty(), "This original browser request has no URL");
        ensure!(
            self.questions
                .url
                .as_ref()
                .is_none_or(|copy| !self.documents.formatting.copy_is_current(copy.id)),
            "The original browser URL copy is still pending"
        );
        self.documents.copy = None;
        self.documents.selection = None;
        self.documents.formatting.end_selection();
        let id = self.documents.formatting.begin_copy()?;
        self.questions.url = Some(UrlCopy {
            card: card.id.clone(),
            id,
            fingerprint: fingerprint(card),
            url: url.clone(),
            offset: 0,
            queued: false,
        });
        let copy = self.questions.url.take().expect("captured URL copy");
        self.question_url_status(&copy, "Copying the original browser URL…".into());
        self.questions.url = Some(copy);
        Ok(())
    }

    pub(super) fn drive_question_url(&mut self) -> Result<bool> {
        let Some(copy) = &mut self.questions.url else {
            return Ok(false);
        };
        if !self.documents.formatting.copy_is_current(copy.id) {
            self.questions.url = None;
            return Ok(false);
        }
        if copy.queued {
            return Ok(false);
        }
        let mut end = copy.url.len().min(copy.offset + 64 * 1024);
        while !copy.url.is_char_boundary(end) {
            end -= 1;
        }
        let finish = end == copy.url.len();
        if self.documents.formatting.literal_copy(
            copy.id,
            copy.offset,
            &copy.url[copy.offset..end],
            finish,
        )? {
            copy.offset = end;
            copy.queued = finish;
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) fn question_url_copied(&mut self, id: u64, backend: ClipboardBackend) -> bool {
        if !self
            .questions
            .url
            .as_ref()
            .is_some_and(|copy| copy.id == id)
        {
            return false;
        }
        let copy = self.questions.url.take().expect("original URL copy");
        if self.documents.formatting.copy_is_current(copy.id) {
            self.documents.formatting.cancel_copy();
            self.question_url_status(
                &copy,
                format!(
                "Browser link copied ({}); approve only if you consent to this server's request",
                clipboard_backend_label(backend)
            ),
            );
        }
        true
    }

    pub(super) fn question_url_failed(&mut self, id: u64, error: &TransferError) -> bool {
        if !self
            .questions
            .url
            .as_ref()
            .is_some_and(|copy| copy.id == id)
        {
            return false;
        }
        let copy = self.questions.url.take().expect("original URL copy");
        if self.documents.formatting.copy_is_current(copy.id) {
            self.documents.formatting.cancel_copy();
            self.question_url_status(
                &copy,
                format!("Could not copy the original browser URL: {error:?}"),
            );
        }
        true
    }

    fn question_url_status(&mut self, copy: &UrlCopy, message: String) {
        let current = self
            .client
            .session(&self.session)
            .and_then(|replica| replica.approvals().iter().find(|card| card.id == copy.card))
            .is_some_and(|card| fingerprint(card) == copy.fingerprint);
        if !current {
            return;
        }
        if let Some(draft) = self
            .questions
            .drafts
            .get_mut(&copy.card)
            .filter(|draft| draft.fingerprint == copy.fingerprint)
        {
            draft.editor.notice = Some(message.clone());
        }
        self.view.status_hint = Some(message);
    }
}
