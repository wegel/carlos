use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Map, Value};

use crate::backend::{BackendClient, BackendKind};
use crate::protocol::ModelInfo;

const CLAUDE_CONTEXT_WINDOW: u64 = 1_000_000;
pub(crate) const CLAUDE_PENDING_THREAD_ID: &str = "claude-pending-session";

#[derive(Debug, Clone)]
pub(crate) enum ClaudeLaunchMode {
    New,
    Resume(String),
    Continue,
}

#[derive(Debug, Clone)]
struct ClaudeToolCall {
    name: String,
    input: Value,
}

#[derive(Debug, Clone)]
enum ClaudeBlockState {
    Text {
        item_id: String,
        text: String,
    },
    ToolUse {
        item_id: String,
        name: String,
        input_json: String,
    },
}

#[derive(Debug, Default)]
pub(crate) struct ClaudeTranslationState {
    session_id: Option<String>,
    model: Option<String>,
    next_turn_seq: u64,
    current_turn_id: Option<String>,
    current_message_seq: u64,
    current_blocks: HashMap<usize, ClaudeBlockState>,
    tool_calls: HashMap<String, ClaudeToolCall>,
    interrupt_requested: bool,
}

#[derive(Debug, Default)]
pub(crate) struct TranslateOutput {
    pub(crate) lines: Vec<String>,
}

pub(crate) struct ClaudeClient {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    events_tx: mpsc::Sender<String>,
    events_rx: Option<mpsc::Receiver<String>>,
    reader_thread: Option<thread::JoinHandle<()>>,
    next_user_message_seq: AtomicU64,
}

impl ClaudeClient {
    pub(crate) fn start(cwd: &Path, launch_mode: ClaudeLaunchMode) -> Result<Self> {
        let mut command = Command::new("claude");
        command
            .arg("-p")
            .arg("--input-format")
            .arg("stream-json")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose")
            .arg("--include-partial-messages")
            .arg("--permission-mode")
            .arg("bypassPermissions")
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        match launch_mode {
            ClaudeLaunchMode::New => {}
            ClaudeLaunchMode::Resume(session_id) => {
                command.arg("--resume").arg(session_id);
            }
            ClaudeLaunchMode::Continue => {
                command.arg("--continue");
            }
        }

        let mut child = command.spawn().context("failed to spawn `claude`")?;
        let stdin = child.stdin.take().context("missing child stdin")?;
        let stdout = child.stdout.take().context("missing child stdout")?;

        let (events_tx, events_rx) = mpsc::channel::<String>();
        let events_tx_for_thread = events_tx.clone();

        let reader_thread = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            let mut state = ClaudeTranslationState::default();

            loop {
                line.clear();
                let n = match reader.read_line(&mut line) {
                    Ok(n) => n,
                    Err(_) => break,
                };
                if n == 0 {
                    break;
                }

                let trimmed = line.trim_end_matches(['\n', '\r']);
                if trimmed.is_empty() {
                    continue;
                }

                let translated = match translate_claude_line(&mut state, trimmed) {
                    Ok(output) => output,
                    Err(_) => continue,
                };

                for synthetic in translated.lines {
                    let _ = events_tx_for_thread.send(synthetic);
                }
            }
        });

        Ok(Self {
            child,
            stdin: Arc::new(Mutex::new(stdin)),
            events_tx,
            events_rx: Some(events_rx),
            reader_thread: Some(reader_thread),
            next_user_message_seq: AtomicU64::new(1),
        })
    }

    pub(crate) fn synthetic_start_response(&self) -> String {
        json!({
            "jsonrpc": "2.0",
            "result": {
                "thread": {
                    "id": CLAUDE_PENDING_THREAD_ID
                }
            },
        })
        .to_string()
    }

    fn send_stream_user_message(&self, text: &str) -> Result<String> {
        let line = json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": text,
                }]
            }
        })
        .to_string();

        let mut stdin = self
            .stdin
            .lock()
            .map_err(|_| anyhow!("Claude stdin lock poisoned"))?;
        stdin.write_all(line.as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        let user_message_seq = self.next_user_message_seq.fetch_add(1, Ordering::SeqCst);
        let _ = self
            .events_tx
            .send(synthetic_user_message_line(user_message_seq, text));

        Ok(json!({
            "jsonrpc": "2.0",
            "result": {}
        })
        .to_string())
    }

}

impl BackendClient for ClaudeClient {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }

    fn call(&self, method: &str, params: Value, _timeout: Duration) -> Result<String> {
        match method {
            "turn/start" | "turn/steer" => {
                let text = params
                    .get("input")
                    .and_then(Value::as_array)
                    .and_then(|items| items.first())
                    .and_then(|item| item.get("text"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .context("missing turn input text")?;
                self.send_stream_user_message(text)
            }
            "turn/interrupt" => {
                // This branch requires mutable child access through `&self`, so use interior mutability
                // by reusing the process id and emitting a synthetic interrupt completion.
                let pid = self.child.id();
                let status = Command::new("kill")
                    .arg("-INT")
                    .arg(pid.to_string())
                    .status()
                    .context("failed to send SIGINT to Claude")?;
                if !status.success() {
                    bail!("failed to interrupt Claude process");
                }
                let _ = self.events_tx.send(
                    json!({
                        "method": "turn/completed",
                        "params": {
                            "turn": {
                                "id": "claude-turn-interrupted",
                                "status": "interrupted"
                            }
                        }
                    })
                    .to_string(),
                );
                Ok(json!({
                    "jsonrpc": "2.0",
                    "result": {}
                })
                .to_string())
            }
            other => bail!("unsupported Claude backend method: {other}"),
        }
    }

    fn respond(&self, _request_id: &Value, _result: Value) -> Result<()> {
        bail!("Claude backend approvals are not implemented")
    }

    fn respond_error(&self, _request_id: &Value, _code: i64, _message: &str) -> Result<()> {
        bail!("Claude backend approvals are not implemented")
    }

    fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>> {
        self.events_rx
            .take()
            .ok_or_else(|| anyhow!("Claude events receiver already taken"))
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let deadline = Instant::now() + Duration::from_millis(250);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                _ => break,
            }
        }
        let _ = self.reader_thread.take();
    }
}

impl Drop for ClaudeClient {
    fn drop(&mut self) {
        self.stop();
    }
}

pub(crate) fn claude_model_catalog() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            model: "claude-opus-4-6".to_string(),
            display_name: "Claude Opus 4.6".to_string(),
            supported_efforts: Vec::new(),
            default_effort: None,
            is_default: true,
        },
        ModelInfo {
            model: "claude-sonnet-4-6".to_string(),
            display_name: "Claude Sonnet 4.6".to_string(),
            supported_efforts: Vec::new(),
            default_effort: None,
            is_default: false,
        },
        ModelInfo {
            model: "claude-haiku-4-5".to_string(),
            display_name: "Claude Haiku 4.5".to_string(),
            supported_efforts: Vec::new(),
            default_effort: None,
            is_default: false,
        },
    ]
}

pub(crate) fn translate_claude_line(
    state: &mut ClaudeTranslationState,
    line: &str,
) -> Result<TranslateOutput> {
    let parsed: Value = serde_json::from_str(line).context("invalid Claude JSON line")?;
    let root = parsed
        .as_object()
        .context("expected Claude JSON object")?;

    let mut out = TranslateOutput::default();
    match root.get("type").and_then(Value::as_str) {
        Some("system") if root.get("subtype").and_then(Value::as_str) == Some("init") => {
            let session_id = root
                .get("session_id")
                .and_then(Value::as_str)
                .context("missing Claude session_id")?;
            let model = root
                .get("model")
                .and_then(Value::as_str)
                .map(normalize_claude_model_name);
            state.session_id = Some(session_id.to_string());
            state.model = model.clone();
            let mut params = Map::new();
            params.insert("thread".to_string(), json!({ "id": session_id }));
            if let Some(model) = state.model.as_deref() {
                params.insert("model".to_string(), Value::String(model.to_string()));
            }
            out.lines.push(
                json!({
                    "method": "thread/initialized",
                    "params": Value::Object(params),
                })
                .to_string(),
            );
        }
        Some("stream_event") => {
            let event = root
                .get("event")
                .and_then(Value::as_object)
                .context("missing Claude stream event")?;
            match event.get("type").and_then(Value::as_str) {
                Some("message_start") => {
                    if state.current_turn_id.is_none() {
                        state.next_turn_seq = state.next_turn_seq.saturating_add(1);
                        let turn_id = format!("claude-turn-{}", state.next_turn_seq);
                        state.current_turn_id = Some(turn_id.clone());
                        out.lines.push(
                            json!({
                                "method": "turn/started",
                                "params": {
                                    "turn": { "id": turn_id }
                                }
                            })
                            .to_string(),
                        );
                    }
                    state.current_message_seq = state.current_message_seq.saturating_add(1);
                    state.current_blocks.clear();
                }
                Some("content_block_start") => {
                    let index = event
                        .get("index")
                        .and_then(Value::as_u64)
                        .context("missing Claude content block index")?
                        as usize;
                    let block = event
                        .get("content_block")
                        .and_then(Value::as_object)
                        .context("missing Claude content_block")?;
                    match block.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            let item_id =
                                format!("claude-msg-{}-{}", state.current_message_seq, index);
                            state.current_blocks.insert(
                                index,
                                ClaudeBlockState::Text {
                                    item_id: item_id.clone(),
                                    text: String::new(),
                                },
                            );
                            out.lines.push(
                                json!({
                                    "method": "item/started",
                                    "params": {
                                        "item": {
                                            "id": item_id,
                                            "type": "agentMessage"
                                        }
                                    }
                                })
                                .to_string(),
                            );
                        }
                        Some("tool_use") => {
                            let item_id = block
                                .get("id")
                                .and_then(Value::as_str)
                                .map(ToOwned::to_owned)
                                .unwrap_or_else(|| {
                                    format!(
                                        "claude-tool-{}-{}",
                                        state.current_message_seq, index
                                    )
                                });
                            let name = block
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("Tool")
                                .to_string();
                            state.current_blocks.insert(
                                index,
                                ClaudeBlockState::ToolUse {
                                    item_id: item_id.clone(),
                                    name,
                                    input_json: String::new(),
                                },
                            );
                            out.lines.push(
                                json!({
                                    "method": "item/started",
                                    "params": {
                                        "item": {
                                            "id": item_id,
                                            "type": "toolCall"
                                        }
                                    }
                                })
                                .to_string(),
                            );
                        }
                        _ => {}
                    }
                }
                Some("content_block_delta") => {
                    let index = event
                        .get("index")
                        .and_then(Value::as_u64)
                        .context("missing Claude delta index")?
                        as usize;
                    let delta = event
                        .get("delta")
                        .and_then(Value::as_object)
                        .context("missing Claude delta")?;
                    match state.current_blocks.get_mut(&index) {
                        Some(ClaudeBlockState::Text { item_id, text }) => {
                            if delta.get("type").and_then(Value::as_str) == Some("text_delta") {
                                let fragment = delta
                                    .get("text")
                                    .and_then(Value::as_str)
                                    .unwrap_or("");
                                text.push_str(fragment);
                                out.lines.push(
                                    json!({
                                        "method": "item/agentMessage/delta",
                                        "params": {
                                            "itemId": item_id,
                                            "delta": fragment,
                                        }
                                    })
                                    .to_string(),
                                );
                            }
                        }
                        Some(ClaudeBlockState::ToolUse { input_json, .. }) => {
                            if delta.get("type").and_then(Value::as_str) == Some("input_json_delta")
                            {
                                if let Some(fragment) =
                                    delta.get("partial_json").and_then(Value::as_str)
                                {
                                    input_json.push_str(fragment);
                                }
                            }
                        }
                        None => {}
                    }
                }
                Some("content_block_stop") => {
                    let index = event
                        .get("index")
                        .and_then(Value::as_u64)
                        .context("missing Claude content block stop index")?
                        as usize;
                    if let Some(block) = state.current_blocks.remove(&index) {
                        match block {
                            ClaudeBlockState::Text { item_id, text } => {
                                out.lines.push(
                                    json!({
                                        "method": "item/completed",
                                        "params": {
                                            "item": {
                                                "id": item_id,
                                                "type": "agentMessage",
                                                "text": text,
                                            }
                                        }
                                    })
                                    .to_string(),
                                );
                            }
                            ClaudeBlockState::ToolUse {
                                item_id,
                                name,
                                input_json,
                            } => {
                                let input = parse_partial_json_object(&input_json);
                                state.tool_calls.insert(
                                    item_id.clone(),
                                    ClaudeToolCall {
                                        name: name.clone(),
                                        input: Value::Object(input.clone()),
                                    },
                                );
                                out.lines.push(
                                    json!({
                                        "method": "item/completed",
                                        "params": {
                                            "item": {
                                                "id": item_id,
                                                "type": "toolCall",
                                                "tool": name,
                                                "name": name,
                                                "input": input,
                                            }
                                        }
                                    })
                                    .to_string(),
                                );
                            }
                        }
                    }
                }
                Some("message_delta") => {
                    if let Some(usage) = event.get("usage").and_then(Value::as_object) {
                        out.lines.push(synthetic_token_usage_line(
                            usage,
                            state.model.as_deref(),
                        ));
                    }
                }
                Some("message_stop") => {}
                _ => {}
            }
        }
        Some("user") => {
            let message = root
                .get("message")
                .and_then(Value::as_object)
                .filter(|msg| msg.get("role").and_then(Value::as_str) == Some("user"));
            if let Some(message) = message {
                if let Some(content) = message.get("content").and_then(Value::as_array) {
                    for part in content {
                        if part.get("type").and_then(Value::as_str) != Some("tool_result") {
                            continue;
                        }
                        let tool_use_id = part
                            .get("tool_use_id")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        let Some(tool_call) = state.tool_calls.remove(tool_use_id) else {
                            continue;
                        };
                        if let Some(line) = synthetic_tool_result_line(&tool_call, tool_use_id, part, root.get("tool_use_result")) {
                            out.lines.push(line);
                        }
                    }
                }
            }
        }
        Some("result") => {
            if let Some(usage) = root.get("usage").and_then(Value::as_object) {
                out.lines
                    .push(synthetic_token_usage_line(usage, state.model.as_deref()));
            }
            if let Some(turn_id) = state.current_turn_id.take() {
                let status = if state.interrupt_requested
                    || root.get("terminal_reason").and_then(Value::as_str) == Some("interrupted")
                {
                    state.interrupt_requested = false;
                    "interrupted"
                } else {
                    "completed"
                };
                out.lines.push(
                    json!({
                        "method": "turn/completed",
                        "params": {
                            "turn": {
                                "id": turn_id,
                                "status": status,
                            }
                        }
                    })
                    .to_string(),
                );
                state.current_blocks.clear();
            }
        }
        _ => {}
    }

    Ok(out)
}

fn normalize_claude_model_name(raw: &str) -> String {
    raw.split('[').next().unwrap_or(raw).trim().to_string()
}

fn parse_partial_json_object(input_json: &str) -> Map<String, Value> {
    if input_json.trim().is_empty() {
        return Map::new();
    }
    serde_json::from_str::<Map<String, Value>>(input_json).unwrap_or_default()
}

pub(crate) fn synthetic_user_message_line(seq: u64, text: &str) -> String {
    json!({
        "method": "item/started",
        "params": {
            "item": {
                "id": format!("claude-user-{seq}"),
                "type": "userMessage",
                "content": [{
                    "type": "text",
                    "text": text,
                }]
            }
        }
    })
    .to_string()
}

fn synthetic_token_usage_line(
    usage: &Map<String, Value>,
    model: Option<&str>,
) -> String {
    let total_tokens = [
        usage.get("input_tokens").and_then(Value::as_u64),
        usage
            .get("cache_creation_input_tokens")
            .and_then(Value::as_u64),
        usage.get("cache_read_input_tokens").and_then(Value::as_u64),
        usage.get("output_tokens").and_then(Value::as_u64),
    ]
    .into_iter()
    .flatten()
    .sum::<u64>();

    let context_window = match model.unwrap_or_default() {
        "claude-haiku-4-5" | "claude-sonnet-4-6" | "claude-opus-4-6" => CLAUDE_CONTEXT_WINDOW,
        _ => CLAUDE_CONTEXT_WINDOW,
    };

    json!({
        "method": "thread/tokenUsage/updated",
        "params": {
            "tokenUsage": {
                "modelContextWindow": context_window,
                "last": {
                    "totalTokens": total_tokens,
                },
                "total": {
                    "totalTokens": total_tokens,
                }
            }
        }
    })
    .to_string()
}

fn synthetic_tool_result_line(
    tool_call: &ClaudeToolCall,
    tool_use_id: &str,
    part: &Value,
    tool_use_result: Option<&Value>,
) -> Option<String> {
    let lower = tool_call.name.to_ascii_lowercase();
    let is_error = part
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let content = part.get("content")?;
    let content_text = value_to_string(content);

    if lower == "bash" {
        let command = tool_call
            .input
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("");
        let (stdout, stderr, interrupted) = tool_use_result
            .and_then(Value::as_object)
            .map(|obj| {
                let stdout = obj.get("stdout").and_then(Value::as_str).unwrap_or("");
                let stderr = obj.get("stderr").and_then(Value::as_str).unwrap_or("");
                let interrupted = obj
                    .get("interrupted")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                (stdout.to_string(), stderr.to_string(), interrupted)
            })
            .unwrap_or_else(|| (String::new(), String::new(), false));

        let raw_output = if !stdout.is_empty() || !stderr.is_empty() {
            match (stdout.trim_end(), stderr.trim_end()) {
                ("", stderr) => stderr.to_string(),
                (stdout, "") => stdout.to_string(),
                (stdout, stderr) => format!("{stdout}\n{stderr}"),
            }
        } else {
            content_text.clone()
        };

        let exit_code = if is_error || interrupted { 1 } else { 0 };
        let formatted_output = if raw_output.trim().is_empty() {
            format!("$ {command}\nexit code: {exit_code}")
        } else {
            format!("$ {command}\n{raw_output}\n\nexit code: {exit_code}")
        };

        let mut item = Map::new();
        item.insert(
            "id".to_string(),
            Value::String(format!("{tool_use_id}:result")),
        );
        item.insert("type".to_string(), Value::String("toolResult".to_string()));
        item.insert("tool".to_string(), Value::String(tool_call.name.clone()));
        item.insert("name".to_string(), Value::String(tool_call.name.clone()));
        item.insert("output".to_string(), Value::String(formatted_output));
        item.insert("command".to_string(), Value::String(command.to_string()));
        if is_probably_diff_text(&raw_output) {
            item.insert("diff".to_string(), Value::String(raw_output));
        }

        return Some(
            json!({
                "method": "item/completed",
                "params": {
                    "item": Value::Object(item)
                }
            })
            .to_string(),
        );
    }

    if !is_error && lower == "read" {
        return None;
    }

    if !is_error && lower != "write" && lower != "edit" {
        return None;
    }

    let mut item = Map::new();
    item.insert(
        "id".to_string(),
        Value::String(format!("{tool_use_id}:result")),
    );
    item.insert("type".to_string(), Value::String("toolResult".to_string()));
    item.insert("tool".to_string(), Value::String(tool_call.name.clone()));
    item.insert("name".to_string(), Value::String(tool_call.name.clone()));
    item.insert("input".to_string(), tool_call.input.clone());

    if let Some(result) = tool_use_result.cloned() {
        item.insert("result".to_string(), result);
    }

    if !content_text.trim().is_empty() {
        item.insert("output".to_string(), Value::String(content_text.clone()));
        if is_probably_diff_text(&content_text) {
            item.insert("diff".to_string(), Value::String(content_text));
        }
    } else if is_error {
        item.insert(
            "output".to_string(),
            Value::String("tool failed".to_string()),
        );
    }

    Some(
        json!({
            "method": "item/completed",
            "params": {
                "item": Value::Object(item)
            }
        })
        .to_string(),
    )
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(value_to_string)
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn is_probably_diff_text(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with("diff --git ")
        || trimmed.starts_with("@@ ")
        || (trimmed.contains("\n@@ ")
            && (trimmed.contains("\n+++ ") || trimmed.contains("\n--- ")))
        || (trimmed.contains('\n') && trimmed.contains("\n+++ ") && trimmed.contains("\n--- "))
}
