//! Borrow chronological client rows and private echoes for the shared lazy render cache.

use carlos_client::{Bodies, Client, LocalOperation, Replica};
use carlos_core::session::models::{MessageKind, Role};
use carlos_proto::ids::SessionId;
use carlos_proto::source::INLINE_SOURCE_BYTES;

use super::bodies::DocumentReads;

use super::super::render_cache_state::TranscriptSource;
use super::super::render_entry_cache::{PreparedRow, RenderKey, RowFormat, RowSource};
use super::super::transcript_input::RenderInput;
use crate::app::flow_transcript::EntryContext;

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
    fn row(&self, index: usize) -> Option<RowSource<'_>> {
        if let Some(entry) = self.replica.entry_at(index) {
            return Some(RowSource {
                key: RenderKey::Entry(carlos_core::session::entry_id::EntryId::from_wire(
                    &entry.id,
                )),
                generation: entry
                    .source
                    .as_ref()
                    .map(|source| source.revision)
                    .or_else(|| entry.tool.as_ref().map(|tool| tool.output_revision)),
                input: self.input(index),
                facts: RenderInput::client_facts(entry),
            });
        }
        let echo = self
            .echoes
            .get(index.checked_sub(self.replica.entry_count())?)?;
        let input = self.input(index)?;
        Some(RowSource {
            key: RenderKey::Echo(echo.operation.clone()),
            generation: None,
            input: Some(input),
            facts: input,
        })
    }
    fn prepared(
        &self,
        index: usize,
        format: RowFormat,
        previous: Option<EntryContext>,
    ) -> Option<PreparedRow> {
        let entry = self.replica.entry_at(index)?;
        let window = self
            .documents
            .token(&entry.id)
            .and_then(|token| self.bodies.source(token));
        let source = entry.source.as_ref().or_else(|| window?.descriptor())?;
        self.documents.formatting.prepared(
            &entry.id,
            RenderInput::client_facts(entry),
            source,
            format,
            previous,
        )
    }

    fn input(&self, index: usize) -> Option<RenderInput<'_>> {
        if let Some(entry) = self.replica.entry_at(index) {
            let source = self
                .documents
                .token(&entry.id)
                .and_then(|token| self.bodies.source(token));
            let text = source.and_then(|source| {
                let descriptor = source.descriptor()?;
                if descriptor.bytes.get() > INLINE_SOURCE_BYTES as u64 {
                    return None;
                }
                Some((descriptor, source.document()?))
            });
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
