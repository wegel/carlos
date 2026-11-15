//! Borrow exact offline row originals through the same source interface as the daemon renderer.

use carlos_core::perf_fixture::PerfRow;
use carlos_core::session::entry_id::EntryId;

use super::super::render_cache_state::TranscriptSource;
use super::super::render_entry_cache::{RenderKey, RowSource};
use super::super::transcript_input::RenderInput;

impl TranscriptSource for [PerfRow] {
    fn len(&self) -> usize {
        self.len()
    }

    fn row(&self, index: usize) -> Option<RowSource<'_>> {
        let row = self.get(index)?;
        let input = RenderInput::from_client(
            &row.entry,
            row.source
                .as_ref()
                .map(|(source, text)| (source, text.as_str())),
        );
        Some(RowSource {
            key: RenderKey::Entry(EntryId::from_wire(&row.entry.id)),
            generation: row.source.as_ref().map(|(source, _)| source.revision),
            input,
            facts: RenderInput::client_facts(&row.entry),
        })
    }
}
