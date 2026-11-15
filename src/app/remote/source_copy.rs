//! Keep one explicit original-answer copy private while its bounded source reads finish.

use carlos_proto::ids::EntryId;
use carlos_proto::source::Source;

use super::Device;
use crate::app::layout_state::TranscriptTheme;
use crate::app::render_entry_cache::RowFormat;
use crate::app::transcript_body::formatter::{Facts, View};
use crate::app::transcript_input::RenderInput;
use crate::clipboard::{clipboard_backend_label, ClipboardBackend};

/// Capture one requested answer prefix without retaining another complete source string.
pub(super) struct CopySource {
    pub entry: EntryId,
    pub source: Source,
    pub queued: Option<u64>,
}

impl Device {
    /// Release private copy priority when its source failed or its captured revision disappeared.
    pub(super) fn cancel_unavailable_copy(&mut self) -> bool {
        let original = self.documents.copy.as_ref().map(|copy| &copy.entry);
        let selected = self
            .documents
            .selection
            .as_ref()
            .and_then(|copy| copy.entry());
        let entry = original.or(selected);
        let unavailable = entry.is_some_and(|id| {
            self.documents.failed(id)
                || self
                    .client
                    .session(&self.session)
                    .is_none_or(|replica| replica.entry(id).is_none())
        });
        let replaced = selected.is_some_and(|id| {
            let revision = self
                .client
                .session(&self.session)
                .and_then(|replica| replica.entry(id))
                .and_then(|entry| {
                    entry
                        .source
                        .as_ref()
                        .map(|source| source.revision)
                        .or_else(|| entry.tool.as_ref().map(|tool| tool.output_revision))
                });
            revision
                != self
                    .documents
                    .selection
                    .as_ref()
                    .and_then(|copy| copy.revision())
        });
        if !unavailable && !replaced {
            return false;
        }
        self.documents.copy = None;
        self.documents.selection = None;
        self.documents.formatting.end_selection();
        if replaced {
            self.view.status_hint = Some("Selected source changed; select it again".into());
        } else if self.view.status_hint.as_deref().is_none_or(|hint| {
            matches!(
                hint,
                "Loading original answer for copy…" | "Copying selected text…"
            )
        }) {
            self.view.status_hint = Some("Source failed; F6 retries its read".into());
        }
        true
    }

    /// Capture the answer's exact prefix at the explicit shortcut, including offline accepted bytes.
    pub(super) fn copy_original(&mut self, entry: EntryId, source: Source) {
        self.documents.selection = None;
        self.documents.formatting.end_selection();
        self.documents.copy = Some(CopySource {
            entry,
            source,
            queued: None,
        });
        self.view.status_hint = Some("Loading original answer for copy…".into());
    }

    /// Load a copy-only source without constructing rows that nobody requested.
    pub(super) fn copy_view(&mut self) -> Option<(EntryId, View)> {
        let copy = self.documents.copy.as_ref()?;
        let entry = self.client.session(&self.session)?.entry(&copy.entry)?;
        if entry.source.as_ref().is_none_or(|source| {
            source.revision != copy.source.revision || source.bytes < copy.source.bytes
        }) {
            self.documents.copy = None;
            self.view.status_hint = Some("Answer changed; copy it again".into());
            return None;
        }
        Some((
            copy.entry.clone(),
            View {
                target: Default::default(),
                facts: Facts::from_input(RenderInput::client_facts(entry)),
                format: RowFormat {
                    width: 8,
                    compact: false,
                    theme: TranscriptTheme::Gutter,
                    hidden: false,
                },
                previous: None,
                first: 0,
                length: 0,
            },
        ))
    }

    /// Queue one private clipboard write behind all accepted bytes of the requested prefix.
    pub(super) fn drive_original_copy(&mut self) -> anyhow::Result<()> {
        let Some(copy) = &mut self.documents.copy else {
            return Ok(());
        };
        if copy.queued.is_none()
            && self
                .documents
                .formatting
                .token()
                .is_some_and(|token| token.entry() == &copy.entry)
        {
            copy.queued = Some(self.documents.formatting.copy(copy.source.clone())?);
        }
        Ok(())
    }

    /// A late older copy cannot replace the status of a newer explicit shortcut.
    pub(super) fn copied_original(&mut self, id: u64, backend: ClipboardBackend) {
        if self
            .documents
            .copy
            .as_ref()
            .is_some_and(|copy| copy.queued == Some(id))
        {
            self.documents.copy = None;
            self.view.status_hint = Some(format!(
                "copied last assistant message ({})",
                clipboard_backend_label(backend)
            ));
        }
    }
}
