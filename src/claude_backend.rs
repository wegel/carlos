use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Map, Value};

use crate::backend::{BackendClient, BackendKind};
use crate::protocol::ModelInfo;

const CLAUDE_CONTEXT_WINDOW: u64 = 1_000_000;
pub(crate) const CLAUDE_PENDING_THREAD_ID: &str = "claude-pending-session";
const CLAUDE_EXIT_PLAN_REQUEST_METHOD: &str = "claude/exitPlan/requestApproval";
const CLAUDE_EXIT_PLAN_FALLBACK_TEXT: &str = "Exit plan mode?";

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
struct ClaudeAllowedPrompt {
    prompt: String,
    tool: Option<String>,
}

#[derive(Debug, Clone)]
struct ClaudeExitPlanApproval {
    tool_use_id: String,
    plan: String,
    plan_file_path: Option<String>,
    allowed_prompts: Vec<ClaudeAllowedPrompt>,
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

#[derive(Debug, Clone)]
pub(crate) struct ClaudeLocalHistory {
    pub(crate) session_id: String,
    pub(crate) thread: Value,
    pub(crate) imported_item_count: usize,
    pub(crate) pending_approval_request: Option<String>,
}

pub(crate) struct ClaudeClient {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    events_tx: mpsc::Sender<String>,
    events_rx: Option<mpsc::Receiver<String>>,
    reader_thread: Option<thread::JoinHandle<()>>,
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
        })
    }

    pub(crate) fn synthetic_start_response(
        &self,
        thread_id: &str,
        history_thread: Option<&Value>,
    ) -> String {
        let thread = history_thread
            .cloned()
            .unwrap_or_else(|| json!({ "id": thread_id }));
        json!({
            "jsonrpc": "2.0",
            "result": {
                "thread": thread
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

    fn respond(&self, request_id: &Value, result: Value) -> Result<()> {
        let backend = request_id
            .get("backend")
            .and_then(Value::as_str)
            .unwrap_or("");
        let kind = request_id.get("kind").and_then(Value::as_str).unwrap_or("");

        if backend == "claude" && kind == "exitPlanMode" {
            let _tool_use_id = request_id
                .get("toolUseId")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .context("missing Claude approval toolUseId")?;
            let decision = result
                .get("decision")
                .and_then(Value::as_str)
                .context("missing Claude approval decision")?;
            let follow_up = match decision {
                "accept" => "The plan is approved. Continue with the planned implementation now.",
                "decline" => {
                    "Do not exit plan mode yet. Stay in plan mode, revise the plan, and then present an updated plan for approval."
                }
                "cancel" => "Cancel the exit from plan mode and stay in plan mode.",
                other => bail!("unsupported Claude approval decision: {other}"),
            };
            self.send_stream_user_message(follow_up)?;
            return Ok(());
        }

        bail!("unsupported Claude approval request: {request_id}")
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

fn claude_projects_root() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".claude").join("projects"))
}

pub(crate) fn claude_project_dir_name(cwd: &Path) -> String {
    cwd.to_string_lossy()
        .chars()
        .map(|ch| if ch == '/' || ch == '\\' { '-' } else { ch })
        .collect()
}

fn find_session_file_for_resume(
    projects_root: &Path,
    cwd: &Path,
    session_id: &str,
) -> Option<PathBuf> {
    let file_name = format!("{session_id}.jsonl");
    let preferred = projects_root
        .join(claude_project_dir_name(cwd))
        .join(&file_name);
    if preferred.is_file() {
        return Some(preferred);
    }

    let entries = fs::read_dir(projects_root).ok()?;
    for entry in entries.flatten() {
        let candidate = entry.path().join(&file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn user_message_item(text: &str) -> Value {
    json!({
        "type": "userMessage",
        "content": [{
            "type": "text",
            "text": text,
        }]
    })
}

fn agent_message_item(text: &str) -> Value {
    json!({
        "type": "agentMessage",
        "text": text,
    })
}

fn tool_call_item(tool_use_id: &str, tool_call: &ClaudeToolCall) -> Value {
    json!({
        "id": tool_use_id,
        "type": "toolCall",
        "tool": tool_call.name,
        "name": tool_call.name,
        "input": tool_call.input,
    })
}

fn claude_exit_plan_request_id(tool_use_id: &str) -> Value {
    json!({
        "backend": "claude",
        "kind": "exitPlanMode",
        "toolUseId": tool_use_id,
    })
}

fn claude_exit_plan_approval_from_tool_call(
    tool_call: &ClaudeToolCall,
    tool_use_id: &str,
    part: &Value,
) -> Option<ClaudeExitPlanApproval> {
    if tool_call.name != "ExitPlanMode" {
        return None;
    }

    let is_error = part
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !is_error {
        return None;
    }

    let content_text = value_to_string(part.get("content")?);
    if !content_text.contains(CLAUDE_EXIT_PLAN_FALLBACK_TEXT) {
        return None;
    }

    let plan = tool_call
        .input
        .get("plan")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())?
        .to_string();
    let plan_file_path = tool_call
        .input
        .get("planFilePath")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);
    let allowed_prompts = tool_call
        .input
        .get("allowedPrompts")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    let obj = entry.as_object()?;
                    let prompt = obj
                        .get("prompt")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())?
                        .to_string();
                    let tool = obj
                        .get("tool")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(ToOwned::to_owned);
                    Some(ClaudeAllowedPrompt { prompt, tool })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(ClaudeExitPlanApproval {
        tool_use_id: tool_use_id.to_string(),
        plan,
        plan_file_path,
        allowed_prompts,
    })
}

fn claude_exit_plan_request_line(approval: &ClaudeExitPlanApproval) -> String {
    let allowed_prompts = approval
        .allowed_prompts
        .iter()
        .map(|prompt| {
            json!({
                "prompt": prompt.prompt,
                "tool": prompt.tool,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "jsonrpc": "2.0",
        "id": claude_exit_plan_request_id(&approval.tool_use_id),
        "method": CLAUDE_EXIT_PLAN_REQUEST_METHOD,
        "params": {
            "toolUseId": approval.tool_use_id,
            "plan": approval.plan,
            "planFilePath": approval.plan_file_path,
            "allowedPrompts": allowed_prompts,
        }
    })
    .to_string()
}

fn fallback_tool_result_item(tool_use_id: &str, part: &Value) -> Option<Value> {
    let content_text = value_to_string(part.get("content")?);
    if content_text.trim().is_empty() {
        return None;
    }
    Some(json!({
        "id": format!("{tool_use_id}:result"),
        "type": "toolResult",
        "output": content_text,
    }))
}

fn append_assistant_history_record(
    record: &Value,
    pending_tool_calls: &mut HashMap<String, ClaudeToolCall>,
    items: &mut Vec<Value>,
    pending_exit_plan_approval: &mut Option<ClaudeExitPlanApproval>,
) {
    let Some(message) = record.get("message").and_then(Value::as_object) else {
        return;
    };
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return;
    }
    let Some(content) = message.get("content").and_then(Value::as_array) else {
        return;
    };

    for part in content {
        match part.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    if !text.trim().is_empty() {
                        *pending_exit_plan_approval = None;
                        items.push(agent_message_item(text));
                    }
                }
            }
            Some("tool_use") => {
                *pending_exit_plan_approval = None;
                let tool_use_id = part
                    .get("id")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| {
                        format!("claude-history-tool-{}", pending_tool_calls.len() + 1)
                    });
                let tool_call = ClaudeToolCall {
                    name: part
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("Tool")
                        .to_string(),
                    input: part.get("input").cloned().unwrap_or_else(|| json!({})),
                };
                items.push(tool_call_item(&tool_use_id, &tool_call));
                pending_tool_calls.insert(tool_use_id, tool_call);
            }
            _ => {}
        }
    }
}

fn append_user_history_record(
    record: &Value,
    pending_tool_calls: &mut HashMap<String, ClaudeToolCall>,
    items: &mut Vec<Value>,
    pending_exit_plan_approval: &mut Option<ClaudeExitPlanApproval>,
) {
    let Some(message) = record.get("message").and_then(Value::as_object) else {
        return;
    };
    if message.get("role").and_then(Value::as_str) != Some("user") {
        return;
    }

    match message.get("content") {
        Some(Value::String(text)) => {
            if !text.trim().is_empty() {
                *pending_exit_plan_approval = None;
                items.push(user_message_item(text));
            }
        }
        Some(Value::Array(parts)) => {
            let text_parts: Vec<&str> = parts
                .iter()
                .filter(|part| part.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .filter(|text| !text.trim().is_empty())
                .collect();
            if !text_parts.is_empty() {
                *pending_exit_plan_approval = None;
                items.push(user_message_item(&text_parts.join("\n")));
            }

            let tool_use_result = record
                .get("toolUseResult")
                .or_else(|| record.get("tool_use_result"));
            for part in parts {
                if part.get("type").and_then(Value::as_str) != Some("tool_result") {
                    continue;
                }
                let tool_use_id = part
                    .get("tool_use_id")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if tool_use_id.is_empty() {
                    continue;
                }

                let (item, exit_plan_approval) = if let Some(tool_call) =
                    pending_tool_calls.remove(tool_use_id)
                {
                    let approval =
                        claude_exit_plan_approval_from_tool_call(&tool_call, tool_use_id, part);
                    let item =
                        synthetic_tool_result_item(&tool_call, tool_use_id, part, tool_use_result);
                    (item, approval)
                } else {
                    (fallback_tool_result_item(tool_use_id, part), None)
                };
                if let Some(item) = item {
                    items.push(item);
                }
                if let Some(approval) = exit_plan_approval {
                    *pending_exit_plan_approval = Some(approval);
                } else {
                    *pending_exit_plan_approval = None;
                }
            }
        }
        _ => {}
    }
}

fn parse_local_history_from_file(path: &Path, session_id: &str) -> Result<ClaudeLocalHistory> {
    let file = File::open(path)
        .with_context(|| format!("failed to open Claude session file {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut items = Vec::new();
    let mut pending_tool_calls = HashMap::new();
    let mut pending_exit_plan_approval = None;
    let mut saw_malformed_record = false;

    for line in reader.lines() {
        let Ok(line) = line else {
            saw_malformed_record = true;
            continue;
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Value>(trimmed) else {
            saw_malformed_record = true;
            continue;
        };
        match record.get("type").and_then(Value::as_str) {
            Some("assistant") => append_assistant_history_record(
                &record,
                &mut pending_tool_calls,
                &mut items,
                &mut pending_exit_plan_approval,
            ),
            Some("user") => append_user_history_record(
                &record,
                &mut pending_tool_calls,
                &mut items,
                &mut pending_exit_plan_approval,
            ),
            _ => {}
        }
    }

    if saw_malformed_record {
        bail!("Claude session file contained malformed JSONL records");
    }

    let imported_item_count = items.len();
    Ok(ClaudeLocalHistory {
        session_id: session_id.to_string(),
        thread: json!({
            "id": session_id,
            "turns": [{
                "items": items,
            }]
        }),
        imported_item_count,
        pending_approval_request: pending_exit_plan_approval
            .as_ref()
            .map(claude_exit_plan_request_line),
    })
}

pub(crate) fn load_claude_local_history_from_projects_root(
    projects_root: &Path,
    cwd: &Path,
    launch_mode: &ClaudeLaunchMode,
) -> Result<Option<ClaudeLocalHistory>> {
    let session_path = match launch_mode {
        ClaudeLaunchMode::New => return Ok(None),
        ClaudeLaunchMode::Resume(session_id) => {
            find_session_file_for_resume(projects_root, cwd, session_id)
                .map(|path| (session_id.clone(), path))
        }
        // `claude --continue` does not expose its chosen resumed session up front, so
        // preloading local history here risks showing the wrong transcript before the live
        // backend confirms which session it actually continued.
        ClaudeLaunchMode::Continue => return Ok(None),
    };

    let Some((session_id, path)) = session_path else {
        return Ok(None);
    };
    match parse_local_history_from_file(&path, &session_id) {
        Ok(history) => Ok(Some(history)),
        Err(_) => Ok(None),
    }
}

pub(crate) fn load_claude_local_history(
    cwd: &Path,
    launch_mode: &ClaudeLaunchMode,
) -> Result<Option<ClaudeLocalHistory>> {
    let Some(projects_root) = claude_projects_root() else {
        return Ok(None);
    };
    if !projects_root.is_dir() {
        return Ok(None);
    }
    load_claude_local_history_from_projects_root(&projects_root, cwd, launch_mode)
}

pub(crate) fn translate_claude_line(
    state: &mut ClaudeTranslationState,
    line: &str,
) -> Result<TranslateOutput> {
    let parsed: Value = serde_json::from_str(line).context("invalid Claude JSON line")?;
    let root = parsed.as_object().context("expected Claude JSON object")?;

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
                                    format!("claude-tool-{}-{}", state.current_message_seq, index)
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
                                let fragment =
                                    delta.get("text").and_then(Value::as_str).unwrap_or("");
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
                        out.lines
                            .push(synthetic_token_usage_line(usage, state.model.as_deref()));
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
                        let pending_approval =
                            claude_exit_plan_approval_from_tool_call(&tool_call, tool_use_id, part);
                        if let Some(line) = synthetic_tool_result_line(
                            &tool_call,
                            tool_use_id,
                            part,
                            root.get("tool_use_result"),
                        ) {
                            out.lines.push(line);
                        }
                        if let Some(approval) = pending_approval {
                            out.lines.push(claude_exit_plan_request_line(&approval));
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

fn synthetic_token_usage_line(usage: &Map<String, Value>, model: Option<&str>) -> String {
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

fn synthetic_tool_result_item(
    tool_call: &ClaudeToolCall,
    tool_use_id: &str,
    part: &Value,
    tool_use_result: Option<&Value>,
) -> Option<Value> {
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

        return Some(Value::Object(item));
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

    Some(Value::Object(item))
}

fn synthetic_tool_result_line(
    tool_call: &ClaudeToolCall,
    tool_use_id: &str,
    part: &Value,
    tool_use_result: Option<&Value>,
) -> Option<String> {
    let item = synthetic_tool_result_item(tool_call, tool_use_id, part, tool_use_result)?;
    Some(
        json!({
            "method": "item/completed",
            "params": {
                "item": item
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
        || (trimmed.contains("\n@@ ") && (trimmed.contains("\n+++ ") || trimmed.contains("\n--- ")))
        || (trimmed.contains('\n') && trimmed.contains("\n+++ ") && trimmed.contains("\n--- "))
}
