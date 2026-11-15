//! Export complete offline fixtures without returning a mutable session or applying wire page caps.

use std::io::BufRead;
use std::time::{Duration, Instant};

use anyhow::Result;
use carlos_proto::session::Entry;
use carlos_proto::source::Source;
use serde_json::Value;

use super::{populate_synthetic_perf_session, SyntheticPerfSpec};
use crate::session::perf_rows::consume;
use crate::session::state::SessionState;

/// One neutral render row owns its original once, inline or behind its exported source facts.
#[derive(Debug, Clone)]
pub struct PerfRow {
    pub entry: Entry,
    pub source: Option<(Source, String)>,
}

impl PerfRow {
    /// Borrow the one original for byte fidelity and offline layout measurements.
    pub fn text(&self) -> &str {
        self.source
            .as_ref()
            .map_or(self.entry.text.as_str(), |(_, text)| text)
    }
}

/// Full diagnostic data is independent of the bounded live transport's history windows.
pub struct PerfData {
    pub rows: Vec<PerfRow>,
    pub relevant_items: usize,
}

/// Build deterministic fixture text inside core and move only exported rows to the renderer.
pub fn synthetic_perf_rows(spec: SyntheticPerfSpec, mut applied: impl FnMut(Duration)) -> PerfData {
    let mut session = SessionState::new("offline-synthetic".into(), Default::default(), false);
    let started = Instant::now();
    populate_synthetic_perf_session(spec, &mut session);
    applied(started.elapsed());
    let relevant_items = session.messages().len();
    PerfData {
        rows: consume(session),
        relevant_items,
    }
}

/// Read captured response items through core's normal text, diff and read-label grouping helpers.
pub fn replay_perf_rows(
    reader: impl BufRead,
    mut applied: impl FnMut(Duration),
) -> Result<PerfData> {
    let mut session = SessionState::new("offline-capture".into(), Default::default(), false);
    let mut relevant_items = 0;
    for line in reader.lines() {
        let Ok(value) = serde_json::from_str::<Value>(&line?) else {
            continue;
        };
        if value["type"] != "response_item" {
            continue;
        }
        let Some(payload) = value.get("payload") else {
            continue;
        };
        let started = Instant::now();
        if super::replay::append_perf_session_item(&mut session, payload) {
            relevant_items += 1;
            applied(started.elapsed());
        }
    }
    Ok(PerfData {
        rows: consume(session),
        relevant_items,
    })
}

#[cfg(test)]
#[path = "perf_fixture_rows_tests.rs"]
mod tests;
