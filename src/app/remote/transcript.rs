//! Borrow chronological client rows and private echoes for the shared lazy render cache.

use carlos_client::{Bodies, Client, LocalOperation, Replica};
use carlos_core::session::models::{MessageKind, Role};
use carlos_proto::ids::SessionId;

use super::bodies::DocumentReads;

use super::super::render_cache_state::TranscriptSource;
use super::super::transcript_input::RenderInput;

pub(super) struct Transcript<'a> {
    replica: &'a Replica,
    bodies: &'a Bodies,
    documents: &'a DocumentReads,
    echoes: Vec<&'a LocalOperation>,
}

impl<'a> Transcript<'a> {
    pub fn new(
        client: &'a Client,
        session: &SessionId,
        documents: &'a DocumentReads,
    ) -> Option<Self> {
        Some(Self {
            replica: client.session(session)?,
            bodies: client.bodies(),
            documents,
            echoes: client
                .operations()
                .iter()
                .filter(|operation| {
                    operation.session.as_ref() == Some(session) && operation.needs_echo()
                })
                .collect(),
        })
    }
}

impl TranscriptSource for Transcript<'_> {
    fn len(&self) -> usize {
        self.replica.entry_count() + self.echoes.len()
    }
    fn input(&self, index: usize) -> Option<RenderInput<'_>> {
        if let Some(entry) = self.replica.entry_at(index) {
            let source = self
                .documents
                .token(&entry.id)
                .and_then(|token| self.bodies.source(token));
            let text = source.and_then(|source| Some((source.descriptor()?, source.document()?)));
            return RenderInput::from_client(entry, text);
        }
        let echo = self
            .echoes
            .get(index.checked_sub(self.replica.entry_count())?)?;
        Some(RenderInput {
            role: Role::User,
            kind: MessageKind::Plain,
            text: echo.text()?,
            file_path: None,
            label: None,
            label_generated: false,
            failed: false,
        })
    }
}
