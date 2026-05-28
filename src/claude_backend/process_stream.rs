//! Claude CLI stream forwarding, stderr capture, and session misroute reporting.

// --- Imports ---

use std::io::{BufRead, Read};
use std::process::{ChildStderr, ChildStdout, Command};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use serde_json::json;

use super::super::translate::translate_claude_line;
use super::super::types::ClaudeTranslationState;

static CLAUDE_STREAM_INSTANCE_SEQ: AtomicU64 = AtomicU64::new(1);

// --- Stream state ---

pub(super) enum ClaudeStartupEvent {
    StreamReady,
    StreamClosed,
}

fn next_claude_stream_instance_seq() -> u64 {
    CLAUDE_STREAM_INSTANCE_SEQ.fetch_add(1, Ordering::Relaxed)
}

// --- Stdout forwarding ---

pub(super) fn spawn_reader_thread(
    stdout: ChildStdout,
    events_tx: mpsc::Sender<String>,
    session_id: Arc<Mutex<Option<String>>>,
    expected_session_id: Option<String>,
    child_pid: u32,
    misrouted: Arc<AtomicBool>,
    startup_tx: Option<mpsc::Sender<ClaudeStartupEvent>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        let mut state = ClaudeTranslationState {
            stream_instance_seq: next_claude_stream_instance_seq(),
            ..ClaudeTranslationState::default()
        };
        let mut startup_tx = startup_tx;
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => {
                    notify_stream_closed(&mut startup_tx);
                    break;
                }
                _ => {}
            }
            notify_stream_ready(&mut startup_tx);
            let trimmed = line.trim_end_matches(['\n', '\r']);
            if trimmed.is_empty() {
                continue;
            }
            forward_claude_line(
                &events_tx,
                &session_id,
                expected_session_id.as_deref(),
                child_pid,
                &misrouted,
                &mut state,
                trimmed,
            );
            if misrouted.load(Ordering::SeqCst) {
                break;
            }
        }
    })
}

fn notify_stream_closed(startup_tx: &mut Option<mpsc::Sender<ClaudeStartupEvent>>) {
    if let Some(tx) = startup_tx.take() {
        let _ = tx.send(ClaudeStartupEvent::StreamClosed);
    }
}

fn notify_stream_ready(startup_tx: &mut Option<mpsc::Sender<ClaudeStartupEvent>>) {
    if let Some(tx) = startup_tx.take() {
        let _ = tx.send(ClaudeStartupEvent::StreamReady);
    }
}

fn forward_claude_line(
    events_tx: &mpsc::Sender<String>,
    session_id: &Arc<Mutex<Option<String>>>,
    expected_session_id: Option<&str>,
    child_pid: u32,
    misrouted: &Arc<AtomicBool>,
    state: &mut ClaudeTranslationState,
    line: &str,
) {
    let Ok(translated) = translate_claude_line(state, line) else {
        return;
    };
    if let Some(sid) = state
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if reject_misrouted_session(events_tx, expected_session_id, sid, child_pid, misrouted) {
            return;
        }
        update_current_session_id(session_id, sid);
    }
    for synthetic in translated.lines {
        let _ = events_tx.send(synthetic);
    }
}

fn reject_misrouted_session(
    events_tx: &mpsc::Sender<String>,
    expected_session_id: Option<&str>,
    actual: &str,
    child_pid: u32,
    misrouted: &Arc<AtomicBool>,
) -> bool {
    let Some(expected) = expected_session_id else {
        return false;
    };
    if actual == expected {
        return false;
    }
    misrouted.store(true, Ordering::SeqCst);
    report_session_misroute(events_tx, expected, actual, child_pid);
    true
}

fn update_current_session_id(session_id: &Arc<Mutex<Option<String>>>, sid: &str) {
    if let Ok(mut current) = session_id.lock() {
        if current.as_deref() != Some(sid) {
            *current = Some(sid.to_string());
        }
    }
}

fn report_session_misroute(
    events_tx: &mpsc::Sender<String>,
    expected: &str,
    actual: &str,
    child_pid: u32,
) {
    let _ = events_tx.send(session_misroute_event(expected, actual));
    let _ = Command::new("kill")
        .arg("-KILL")
        .arg(child_pid.to_string())
        .status();
}

pub(crate) fn session_misroute_event(expected: &str, actual: &str) -> String {
    let message = format!(
        "Claude resumed session {actual} but {expected} was requested; child killed to avoid \
         writing into the wrong session file. Retry the action."
    );
    json!({
        "method": "error",
        "params": {
            "message": message,
            "expectedSessionId": expected,
            "actualSessionId": actual,
        }
    })
    .to_string()
}

// --- Stderr capture ---

pub(super) fn spawn_stderr_thread(
    stderr: ChildStderr,
    captured_stderr: Arc<Mutex<String>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stderr);
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(count) => capture_stderr_chunk(&captured_stderr, &buf[..count]),
            }
        }
    })
}

fn capture_stderr_chunk(captured_stderr: &Arc<Mutex<String>>, chunk: &[u8]) {
    let text = std::str::from_utf8(chunk)
        .map(str::to_string)
        .unwrap_or_else(|_| String::from_utf8_lossy(chunk).to_string());
    if let Ok(mut captured) = captured_stderr.lock() {
        captured.push_str(&text);
    }
}

// --- Test helpers ---

#[cfg(test)]
pub(crate) fn collect_live_forwarded_lines_for_test(lines: &[&str]) -> Vec<String> {
    let mut state = ClaudeTranslationState {
        stream_instance_seq: next_claude_stream_instance_seq(),
        ..ClaudeTranslationState::default()
    };
    let mut out = Vec::new();
    for line in lines {
        let translated = translate_claude_line(&mut state, line).expect("translate");
        out.extend(translated.lines);
    }
    out
}
