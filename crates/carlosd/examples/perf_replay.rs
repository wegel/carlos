//! Measure shared synthetic source independently from terminal layout and native startup.

mod replay;

use std::time::Instant;

use anyhow::Result;
use carlos_core::perf_fixture::{populate_synthetic_perf_session, SyntheticPerfSpec};
use carlos_core::session::state::SessionState;
use serde_json::json;

fn main() -> Result<()> {
    let turns = std::env::args()
        .nth(1)
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(1000);
    let before = rss();
    let mut session = SessionState::new(
        "synthetic-0000000000000001".into(),
        Default::default(),
        false,
    );
    let start = Instant::now();
    populate_synthetic_perf_session(
        SyntheticPerfSpec {
            seed: 1,
            turns,
            tool_output_lines: 24,
        },
        &mut session,
    );
    let apply_ms = start.elapsed().as_secs_f64() * 1000.0;
    let after_apply = rss();
    let source_rows = session.messages().len();
    let source_bytes: usize = session.messages().iter().map(|row| row.text.len()).sum();
    let measured = replay::measure(session)?;
    assert_eq!(
        measured.rows, source_rows,
        "bounded pages transfer every source row once"
    );
    println!(
        "{}",
        json!({"turns":turns,"seed":1,"tool_lines":24,"source_rows":source_rows,
        "source_bytes":source_bytes,"source_apply_ms":apply_ms,
        "publish_export_encode_ms":measured.encode_ms,"writer_wait_send_ms":measured.write_ms,
        "client_decode_ms":measured.decode_ms,"transfer_wall_ms":measured.wall_ms,
        "wire_bytes":measured.bytes,"frames":measured.frames,"output_chunks":measured.outputs,
        "rss_kib":{"before":before,"after_apply":after_apply,"after_transfer":rss()},
        "scope":"same core synthetic fixture, production page/output types, real Unix pipe; no native startup, manager admission or terminal render; concurrent costs overlap"})
    );
    Ok(())
}

fn rss() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}
