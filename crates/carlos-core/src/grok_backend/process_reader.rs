//! Bound native ACP frames and keep translated facts charged until the session owner handles them.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use serde_json::{json, Value};

use super::process_client::respond_to_agent_request;
use super::process_writer::InputWriter;
use crate::grok_backend::translate::{finish_prompt_turn, translate_grok_notification};
use crate::grok_backend::types::GrokTranslationState;
use crate::protocol_events::EventQueue;

const MAX_FRAME: u64 = 8 * 1024 * 1024;
const STDERR_BYTES: usize = 16 * 1024;

// --- Reader ---

pub(super) fn spawn_reader_thread(
    stdout: std::process::ChildStdout,
    events_tx: EventQueue,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<String>>>>,
    prompt_in_flight: Arc<Mutex<HashSet<u64>>>,
    stdin: Arc<InputWriter>,
    translation: Arc<Mutex<GrokTranslationState>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let reason = loop {
            line.clear();
            let n = match reader.by_ref().take(MAX_FRAME + 1).read_line(&mut line) {
                Ok(n) => n,
                Err(_) => break "Grok stdout read failed",
            };
            if n == 0 {
                break "Grok stdout closed before replying";
            }
            if n as u64 > MAX_FRAME {
                break "Grok stdout frame exceeds eight MiB";
            }
            let trimmed = line.trim_end_matches(['\n', '\r']);
            if trimmed.is_empty() {
                continue;
            }
            if route_line(
                trimmed,
                &events_tx,
                &pending,
                &prompt_in_flight,
                &stdin,
                &translation,
            )
            .is_err()
            {
                break "Grok event backlog exceeded its count or byte budget";
            }
        };
        stdin.close();
        events_tx.terminal(
            json!({"method":"carlos/grok/closed","params":{"message":reason}}).to_string(),
        );
        if let Ok(mut requests) = pending.lock() {
            for (id, reply) in requests.drain() {
                let _ = reply.send(
                    json!({"id":id,"error":{"message":"Grok stdout closed before replying"}})
                        .to_string(),
                );
            }
        }
    })
}

pub(super) fn spawn_stderr_thread(
    stderr: std::process::ChildStderr,
    captured: Arc<Mutex<String>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut bytes = [0u8; 4096];
        loop {
            let count = match reader.read(&mut bytes) {
                Ok(0) | Err(_) => break,
                Ok(count) => count,
            };
            if let Ok(mut captured) = captured.lock() {
                captured.push_str(&String::from_utf8_lossy(&bytes[..count]));
                if captured.len() > STDERR_BYTES {
                    let mut cut = captured.len() - STDERR_BYTES;
                    while !captured.is_char_boundary(cut) {
                        cut += 1;
                    }
                    captured.drain(..cut);
                }
            }
        }
    })
}

fn route_line(
    line: &str,
    events_tx: &EventQueue,
    pending: &Arc<Mutex<HashMap<u64, mpsc::Sender<String>>>>,
    prompt_in_flight: &Arc<Mutex<HashSet<u64>>>,
    stdin: &Arc<InputWriter>,
    translation: &Arc<Mutex<GrokTranslationState>>,
) -> anyhow::Result<()> {
    let Ok(parsed) = serde_json::from_str::<Value>(line) else {
        return Ok(());
    };
    if let Some(id) = parsed.get("id").and_then(Value::as_u64) {
        if parsed.get("method").and_then(Value::as_str).is_some() {
            respond_to_agent_request(&parsed, stdin);
            return Ok(());
        }
        if let Some(last) = take_prompt_id(prompt_in_flight, id) {
            return emit_prompt_finished(&parsed, events_tx, translation, last);
        }
        if let Ok(mut pending) = pending.lock() {
            if let Some(tx) = pending.remove(&id) {
                let _ = tx.send(line.to_string());
            }
        }
        return Ok(());
    }
    let Some(method) = parsed.get("method").and_then(Value::as_str) else {
        return Ok(());
    };
    let params = parsed.get("params").cloned().unwrap_or(Value::Null);
    let Ok(mut state) = translation.lock() else {
        return Ok(());
    };
    for synthetic in translate_grok_notification(&mut state, method, &params).lines {
        events_tx.send(synthetic)?;
    }
    Ok(())
}

fn take_prompt_id(prompt_in_flight: &Arc<Mutex<HashSet<u64>>>, id: u64) -> Option<bool> {
    let Ok(mut prompts) = prompt_in_flight.lock() else {
        return None;
    };
    if !prompts.remove(&id) {
        return None;
    }
    Some(prompts.is_empty())
}

fn emit_prompt_finished(
    parsed: &Value,
    events_tx: &EventQueue,
    translation: &Arc<Mutex<GrokTranslationState>>,
    last: bool,
) -> anyhow::Result<()> {
    let interrupted = parsed
        .get("result")
        .and_then(|result| result.get("stopReason"))
        .and_then(Value::as_str)
        == Some("cancelled")
        || parsed.get("error").is_some();
    if let Some(message) = parsed
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
    {
        events_tx.send(
            json!({
                "method": "error",
                "params": { "message": format!("Grok prompt failed: {message}") },
            })
            .to_string(),
        )?;
    }
    if !last {
        return Ok(());
    }
    let Ok(mut state) = translation.lock() else {
        return Ok(());
    };
    for synthetic in finish_prompt_turn(&mut state, interrupted).lines {
        events_tx.send(synthetic)?;
    }
    Ok(())
}
