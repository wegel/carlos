//! Consume a diagnostic-only session and move each original into neutral client render facts.

use crate::perf_fixture::PerfRow;

use super::event_rows::is_tool;
use super::state::SessionState;

/// Move every complete original without retaining its session owner.
pub(crate) fn consume(mut session: SessionState) -> Vec<PerfRow> {
    let mut rows = Vec::with_capacity(session.messages.len());
    while let Some(message) = session.messages.entries.last() {
        let mut entry = session.export_entry(message);
        let source =
            (is_tool(message) || entry.source.is_some()).then(|| session.export_source(message));
        let original = session
            .messages
            .entries
            .pop()
            .expect("borrowed final row")
            .text;
        let source = if let Some(source) = source {
            Some((source, original))
        } else {
            entry.text = original;
            None
        };
        rows.push(PerfRow { entry, source });
    }
    rows.reverse();
    rows
}
