//! Read selected queue text in checked UTF-8 ranges while editors remain platform-owned.

use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::Command;
use carlos_proto::ids::SessionId;
use carlos_proto::queue::QueueText;

use crate::panels::queue_input::InputRead;
use crate::panels::{PanelKind, QueueInput};
use crate::{Client, ClientError, Effects};

const MAX_NATIVE_QUEUE_BYTES: u64 = 16 * 1024 * 1024;

impl Client {
    /// Request complete selected plain text without ever publishing a partial editor input.
    pub fn begin_queue_input(&mut self, session: &SessionId) -> Result<Effects, ClientError> {
        if !self.queue_is_current(session) {
            return Err(ClientError::StaleQueue);
        }
        let panel = self.queue_panel_mut(session).expect("current queue panel");
        let entry = panel
            .pages
            .selected(panel.selection.as_deref())
            .ok_or_else(|| {
                ClientError::InvalidPayload("Select an existing queued message first".into())
            })?;
        let bytes = entry.text_bytes.ok_or_else(|| ClientError::InvalidPayload(
            "Edit unavailable: this message has attachments, spans or unsupported input; start/delete/reorder remain available".into()))?.get();
        if bytes > MAX_NATIVE_QUEUE_BYTES {
            return Err(ClientError::Capacity(
                "Queue text exceeds the native snapshot ceiling; original input is retained",
            ));
        }
        let read = InputRead {
            entry: entry.id.clone(),
            expected: panel.pages.revision().cloned().expect("current page"),
            bytes,
            text: String::new(),
            request: None,
        };
        let old = panel.input.read.take().and_then(|read| read.request);
        panel.input = Default::default();
        panel.input.read = Some(read);
        panel.dirty = true;
        if let Some(id) = old {
            self.requests.take(&id);
        }
        self.finish_queue_change(session)
    }

    /// Move complete checked text to the platform; the shared cache retains no duplicate editor.
    pub fn take_queue_input(&mut self, session: &SessionId) -> Option<QueueInput> {
        let panel = self.queue_panel_mut(session)?;
        let ready = panel.input.ready.take();
        panel.recount();
        ready
    }

    /// Cancel only this reader's incomplete input without dropping a platform's private draft.
    pub fn cancel_queue_input(&mut self, session: &SessionId) -> Effects {
        let mut effects = Effects::default();
        if let Some(panel) = self.queue_panel_mut(session) {
            let old = panel.input.read.take().and_then(|read| read.request);
            panel.input = Default::default();
            panel.recount();
            if let Some(id) = old {
                self.requests.take(&id);
            }
            effects.panel_changed(session, PanelKind::Queue);
        }
        effects
    }

    pub(crate) fn drive_queue_input(&mut self, session: &SessionId, effects: &mut Effects) {
        let valid = self.queue_input_revision_is_current(session);
        let panel = self.queue_panel_mut(session).expect("open queue reader");
        let Some(read) = &panel.input.read else {
            return;
        };
        if !valid {
            let id = read.request.clone();
            self.fail_queue_input(
                session,
                "Queue changed before full text arrived; refresh and reselect before editing"
                    .into(),
            );
            if let Some(id) = id {
                self.requests.take(&id);
            }
        } else if read.request.is_none() {
            let command = Command::QueueText {
                session: session.clone(),
                expected: read.expected.clone(),
                entry: read.entry.clone(),
                offset: ByteOffset::new(read.text.len() as u64),
                limit: Some(65536),
            };
            match self.read(self.current.expect("ready reader"), command) {
                Ok(outgoing) => {
                    self.queue_panel_mut(session)
                        .expect("open reader")
                        .input
                        .read
                        .as_mut()
                        .expect("incomplete input")
                        .request = Some(outgoing.id());
                    effects.outgoing.push(outgoing);
                }
                Err(error) => self.fail_queue_input(session, error.to_string()),
            }
        }
        effects.panel_changed(session, PanelKind::Queue);
    }

    pub(crate) fn accept_queue_input(&mut self, text: QueueText, effects: &mut Effects) {
        let session = text.session.clone();
        if !self.queue_input_revision_is_current(&session) {
            self.fail_queue_input(
                &session,
                "Queue changed before full text arrived; refresh and reselect before editing"
                    .into(),
            );
        } else {
            let panel = self.queue_panel_mut(&session).expect("owned input");
            let read = panel.input.read.as_mut().expect("owned input read");
            read.request = None;
            if text.offset.get() != read.text.len() as u64
                || text.next_offset.get() > read.bytes
                || !text.complete && text.text.is_empty()
                || text.complete && text.next_offset.get() != read.bytes
            {
                self.fail_queue_input(
                    &session,
                    "Queue text changed or stopped advancing; original input is retained".into(),
                );
            } else {
                read.text.push_str(&text.text);
                if text.complete {
                    let read = panel.input.read.take().expect("complete input");
                    panel.input.ready = Some(QueueInput {
                        entry: read.entry,
                        expected: read.expected,
                        text: read.text,
                    });
                }
            }
        }
        let panel = self.queue_panel_mut(&session).expect("owned input panel");
        panel.dirty = true;
        panel.recount();
        effects.panel_changed(&session, PanelKind::Queue);
    }

    pub(crate) fn fail_queue_input(&mut self, session: &SessionId, message: String) {
        if let Some(panel) = self.queue_panel_mut(session) {
            panel.input.read = None;
            panel.input.ready = None;
            panel.input.error = Some(message);
            panel.dirty = true;
            panel.recount();
        }
    }

    fn queue_input_revision_is_current(&self, session: &SessionId) -> bool {
        let Some(read) = self
            .queue_panel(session)
            .and_then(|panel| panel.input.read.as_ref())
        else {
            return false;
        };
        self.session(session)
            .and_then(|replica| replica.queue())
            .is_some_and(|facts| facts.known && facts.revision == read.expected)
    }
}
