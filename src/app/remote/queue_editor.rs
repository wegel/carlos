//! Read exact selected queue text before creating an editor, preserving opaque native inputs.

use anyhow::{bail, Result};
use carlos_client::RequestFailure;
use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::{QueueEntryId, QueueRevision};
use carlos_proto::queue::QueueText;
use ratatui_textarea::TextArea;

use super::queue::QueueEditor;
use super::transport::Transport;
use super::Device;

const MAX_NATIVE_QUEUE_BYTES: u64 = 16 * 1024 * 1024;

/// Keep the selected input read and its later local editor under one device lifetime.
#[derive(Default)]
pub(super) struct Editors {
    pub(super) draft: Option<QueueEditor>,
    pub(super) loading: Option<EditorRead>,
    pub(super) version: u64,
}

/// Assemble only the selected complete plain-text input, under the existing native snapshot ceiling.
pub(super) struct EditorRead {
    entry: QueueEntryId,
    expected: QueueRevision,
    bytes: u64,
    text: String,
    request: Option<RequestId>,
}

impl Device {
    /// Refuse attachment-rich inputs before requesting any editable source.
    pub(super) fn begin_queue_edit(&mut self) -> Result<()> {
        self.require_current_queue()?;
        let entry = self
            .queue
            .pages
            .selected(self.view.codex_queue.selected.as_deref())
            .ok_or_else(|| anyhow::anyhow!("Select an existing queued message first"))?;
        let bytes = entry.text_bytes.ok_or_else(|| anyhow::anyhow!("Edit unavailable: this message has attachments, spans or unsupported input; start/delete/reorder remain available"))?.get();
        if bytes > MAX_NATIVE_QUEUE_BYTES {
            bail!("Queue text exceeds the native snapshot ceiling; original input is retained");
        }
        self.queue.editing.loading = Some(EditorRead {
            entry: entry.id.clone(),
            expected: self.queue.pages.revision.clone().expect("current page"),
            bytes,
            text: String::new(),
            request: None,
        });
        self.view.codex_queue.error = None;
        Ok(())
    }

    /// Request one UTF-8-safe piece without exposing a partial input as editable text.
    pub(super) fn drive_queue_text(&mut self, link: &mut Transport) -> Result<()> {
        let Some(load) = self.queue.editing.loading.as_ref() else {
            return Ok(());
        };
        if self
            .queue_facts()
            .is_none_or(|facts| !facts.known || facts.revision != load.expected)
        {
            self.queue.editing.loading = None;
            self.view.codex_queue.error = Some(
                "Queue changed before full text arrived; refresh and reselect before editing"
                    .into(),
            );
            return Ok(());
        }
        if load.request.is_some() {
            return Ok(());
        }
        let outgoing = self.client.read(
            link.connection,
            Command::QueueText {
                session: self.session.clone(),
                expected: load.expected.clone(),
                entry: load.entry.clone(),
                offset: ByteOffset::new(load.text.len() as u64),
                limit: Some(65536),
            },
        )?;
        self.queue
            .editing
            .loading
            .as_mut()
            .expect("queue text read")
            .request = Some(outgoing.id());
        link.send(outgoing)
    }

    /// Publish an editor only after every piece matches the original selected id and revision.
    pub(super) fn accept_queue_text(
        &mut self,
        id: Option<RequestId>,
        text: QueueText,
    ) -> Result<bool> {
        let Some(load) = self
            .queue
            .editing
            .loading
            .as_mut()
            .filter(|load| load.request == id && id.is_some())
        else {
            return Ok(false);
        };
        load.request = None;
        if text.entry != load.entry
            || text.revision != load.expected
            || text.offset.get() != load.text.len() as u64
            || text.next_offset.get() > load.bytes
            || !text.complete && text.text.is_empty()
            || text.complete && text.next_offset.get() != load.bytes
        {
            self.queue.editing.loading = None;
            self.view.codex_queue.error =
                Some("Queue text changed or stopped advancing; original input is retained".into());
            return Ok(true);
        }
        load.text.push_str(&text.text);
        if text.complete {
            let load = self
                .queue
                .editing
                .loading
                .take()
                .expect("complete queue text");
            self.queue.editing.version =
                self.queue.editing.version.checked_add(1).ok_or_else(|| {
                    anyhow::anyhow!("Queue editor counter exhausted; reopen the terminal")
                })?;
            self.queue.editing.draft = Some(QueueEditor {
                entry: load.entry,
                expected: load.expected,
                version: self.queue.editing.version,
                editor: TextArea::new(load.text.split('\n').map(str::to_string).collect()),
            });
        }
        Ok(true)
    }

    /// Keep a failed selected read in this panel without recovering incomplete text into the composer.
    pub(super) fn fail_queue_text(&mut self, failure: &RequestFailure) -> bool {
        if self
            .queue
            .editing
            .loading
            .as_ref()
            .is_some_and(|load| load.request == failure.id && failure.id.is_some())
        {
            self.queue.editing.loading = None;
            self.view.codex_queue.error = Some(failure.error.message.clone());
            true
        } else {
            false
        }
    }
}
