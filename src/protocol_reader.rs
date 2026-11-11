//! Read bounded native JSON lines and retain unknown additive Codex events.

use std::io::{BufRead, BufReader};
use std::process::ChildStdout;
use std::sync::Arc;
use std::thread;

use anyhow::{ensure, Context, Result};
use serde_json::Value;

use crate::protocol_state::{request_key, ClientState};
use crate::protocol_writer::MAX_FRAME_BYTES;

/// Bound one off-UI history reply; ordinary events and writes retain their smaller cap.
pub(crate) const MAX_HISTORY_REPLY_BYTES: usize = 64 * 1024 * 1024;

/// Read native stdout without allowing an unterminated frame to grow forever.
pub(crate) fn spawn(
    stdout: ChildStdout,
    state: Arc<ClientState>,
    diagnostics: crate::codex_diagnostics::Diagnostics,
) {
    thread::spawn(move || {
        let result = read(stdout, &state);
        let message = result.err().map_or_else(
            || "Codex app-server stdout closed".into(),
            |error| error.to_string(),
        );
        state.close(&diagnostics.explain(&message), true);
    });
}

fn read(stdout: ChildStdout, state: &ClientState) -> Result<()> {
    let mut reader = BufReader::new(stdout);
    while !state.is_closed() {
        let Some(line) = read_line_bounded(&mut reader, MAX_HISTORY_REPLY_BYTES)? else {
            break;
        };
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            continue;
        }
        route(line, state)?;
    }
    Ok(())
}

/// Parse one complete frame while enforcing the cap before allocating more bytes.
pub(crate) fn read_line(reader: &mut impl BufRead) -> Result<Option<String>> {
    read_line_bounded(reader, MAX_FRAME_BYTES)
}

fn read_line_bounded(reader: &mut impl BufRead, maximum: usize) -> Result<Option<String>> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().context("Read Codex stdout")?;
        if available.is_empty() {
            ensure!(line.is_empty(), "Codex stdout closed during a JSON frame");
            return Ok(None);
        }
        let end = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|index| index + 1);
        let length = end.unwrap_or(available.len());
        ensure!(
            line.len() + length <= maximum,
            "Codex stdout frame exceeds {} MiB",
            maximum / (1024 * 1024)
        );
        line.extend_from_slice(&available[..length]);
        reader.consume(length);
        if end.is_some() {
            return String::from_utf8(line)
                .context("Codex stdout is not UTF-8")
                .map(Some);
        }
    }
}

pub(crate) fn route(line: &str, state: &ClientState) -> Result<()> {
    let parsed: Value = serde_json::from_str(line).context("Codex stdout contains invalid JSON")?;
    if parsed.get("method").is_none() {
        if let Some(key) = parsed.get("id").and_then(request_key) {
            let large = state.finish_large_reply(&key);
            ensure!(
                line.len() <= MAX_FRAME_BYTES || large,
                "Codex stdout frame exceeds 8 MiB without a matching history read"
            );
            if let Some(target) = state.take(&key) {
                state.reply(&key, target, Ok(line.to_string()));
                return Ok(());
            }
            if state.was_issued(&parsed["id"]) {
                return Ok(());
            }
        }
    }
    ensure!(
        line.len() <= MAX_FRAME_BYTES,
        "Codex stdout frame exceeds 8 MiB"
    );
    state.events.send(line.to_string())
}
