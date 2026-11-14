//! Validate native panel replies against their original explicit session, entry and revision.

use carlos_proto::ids::{QueueRevision, SessionId};
use carlos_proto::queue::{QueuePage, QueueText, MAX_PAGE_ENTRIES};
use serde_json::Value;

use crate::client::Client;
use crate::client_replies::parse;
use crate::pending::QueueTextTarget;
use crate::ClientError;

impl Client {
    pub(crate) fn queue_page_reply(
        &self,
        session: SessionId,
        expected: Option<QueueRevision>,
        value: Value,
    ) -> Result<QueuePage, ClientError> {
        let page: QueuePage = parse(value)?;
        if page.session != session {
            return Err(ClientError::ForeignSession);
        }
        if expected
            .as_ref()
            .is_some_and(|expected| expected != &page.queue.revision)
            || page.entries.len() > MAX_PAGE_ENTRIES
            || page.queue.count > 4096
            || page.entries.iter().any(|entry| {
                entry.preview.chars().take(1025).count() > 1024
                    || entry.client_message.chars().take(129).count() > 128
            })
        {
            return Err(ClientError::InvalidPayload(
                "queue page differs from its requested revision or bounded shape".into(),
            ));
        }
        Ok(page)
    }

    pub(crate) fn queue_text_reply(
        &self,
        target: QueueTextTarget,
        value: Value,
    ) -> Result<QueueText, ClientError> {
        let text: QueueText = parse(value)?;
        if text.session != target.session {
            return Err(ClientError::ForeignSession);
        }
        if text.entry != target.entry
            || text.revision != target.expected
            || text.text.len() > 65536
            || text.offset.get() > target.offset.get()
            || target.offset.get() - text.offset.get() > 3
            || text.offset.get().checked_add(text.text.len() as u64) != Some(text.next_offset.get())
        {
            return Err(ClientError::InvalidBody);
        }
        Ok(text)
    }
}
