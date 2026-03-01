use std::collections::VecDeque;
use std::collections::{HashMap, HashSet};
use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::Terminal;
use ratatui_core::style::{Color as CoreColor, Modifier as CoreModifier, Style as CoreStyle};
use ratatui_textarea::{Input as TextInput, Key as TextKey, TextArea};
use serde_json::{json, Value};
use textwrap::{wrap as wrap_text, Options as WrapOptions, WordSplitter};
use tui_markdown::{
    from_str_with_options as markdown_from_str_with_options, Options as MarkdownOptions,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

// Catppuccin Mocha defaults
const COLOR_STEP1: Color = Color::Rgb(17, 17, 27); // crust
const COLOR_STEP2: Color = Color::Rgb(24, 24, 37); // mantle
const COLOR_STEP3: Color = Color::Rgb(30, 30, 46); // base
const COLOR_STEP6: Color = Color::Rgb(49, 50, 68); // surface0
const COLOR_STEP7: Color = Color::Rgb(69, 71, 90); // surface1
const COLOR_STEP8: Color = Color::Rgb(108, 112, 134); // overlay0
const COLOR_PRIMARY: Color = Color::Rgb(203, 166, 247); // mauve
const COLOR_TEXT: Color = Color::Rgb(205, 214, 244); // text
const COLOR_DIM: Color = Color::Rgb(166, 173, 200); // subtext0
const COLOR_OVERLAY: Color = Color::Rgb(17, 17, 27);

const COLOR_ROW_USER: Color = Color::Rgb(34, 36, 54);
const COLOR_ROW_AGENT_OUTPUT: Color = COLOR_ROW_SYSTEM;
const COLOR_ROW_AGENT_THINKING: Color = Color::Rgb(40, 42, 60);
const COLOR_ROW_TOOL_CALL: Color = Color::Rgb(44, 40, 58);
const COLOR_ROW_TOOL_OUTPUT: Color = Color::Rgb(38, 43, 53);
const COLOR_ROW_SYSTEM: Color = COLOR_STEP2;

const COLOR_GUTTER_USER: Color = Color::Rgb(137, 180, 250); // blue
const COLOR_GUTTER_AGENT_OUTPUT: Color = Color::Rgb(166, 227, 161); // green
const COLOR_GUTTER_AGENT_THINKING: Color = Color::Rgb(245, 194, 231); // pink
const COLOR_GUTTER_TOOL_CALL: Color = Color::Rgb(250, 179, 135); // peach
const COLOR_GUTTER_TOOL_OUTPUT: Color = Color::Rgb(250, 179, 135); // peach
const COLOR_GUTTER_SYSTEM: Color = Color::Rgb(137, 220, 235); // sky
const COLOR_DIFF_ADD: Color = Color::Rgb(166, 227, 161); // green
const COLOR_DIFF_REMOVE: Color = Color::Rgb(243, 139, 168); // red
const COLOR_DIFF_HUNK: Color = Color::Rgb(250, 179, 135); // peach
const COLOR_DIFF_HEADER: Color = Color::Rgb(137, 220, 235); // sky

const MSG_TOP: usize = 1; // 1-based row index
const MSG_CONTENT_X: usize = 2; // 0-based x

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    User,
    Assistant,
    Reasoning,
    ToolCall,
    ToolOutput,
    System,
}

#[derive(Debug, Clone)]
struct Message {
    role: Role,
    text: String,
    kind: MessageKind,
    file_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageKind {
    Plain,
    Diff,
}

#[derive(Debug, Clone)]
struct DiffBlock {
    file_path: Option<String>,
    diff: String,
}

#[derive(Debug, Clone)]
struct ThreadSummary {
    id: String,
    preview: String,
    cwd: String,
    updated_at: i64,
}

#[derive(Debug, Clone, Copy)]
struct Selection {
    anchor_x: usize, // 1-based, content-relative cell column
    anchor_y: usize, // 1-based screen row
    focus_x: usize,
    focus_y: usize,
    dragging: bool,
}

#[derive(Debug, Clone)]
struct StyledSegment {
    text: String,
    style: Style,
}

#[derive(Debug, Clone)]
struct RenderedLine {
    text: String,
    styled_segments: Vec<StyledSegment>,
    role: Role,
    separator: bool,
    cells: usize,
    soft_wrap_to_next: bool,
}

#[derive(Debug, Clone, Copy)]
struct TerminalSize {
    width: usize,
    height: usize,
}

#[derive(Debug)]
struct AppState {
    thread_id: String,
    active_turn_id: Option<String>,
    messages: Vec<Message>,
    agent_item_to_index: HashMap<String, usize>,
    turn_diff_to_index: HashMap<String, usize>,

    input: TextArea<'static>,
    status: String,

    scroll_top: usize,
    auto_follow_bottom: bool,
    selection: Option<Selection>,
    show_help: bool,
}

impl AppState {
    fn new(thread_id: String) -> Self {
        Self {
            thread_id,
            active_turn_id: None,
            messages: Vec::new(),
            agent_item_to_index: HashMap::new(),
            turn_diff_to_index: HashMap::new(),
            input: make_input_area(),
            status: String::new(),
            scroll_top: 0,
            auto_follow_bottom: true,
            selection: None,
            show_help: false,
        }
    }

    fn set_status(&mut self, s: impl Into<String>) {
        self.status = s.into();
    }

    fn input_is_empty(&self) -> bool {
        self.input.is_empty()
    }

    fn input_text(&self) -> String {
        self.input.lines().join("\n")
    }

    fn clear_input(&mut self) {
        self.input = make_input_area();
    }

    fn append_message(&mut self, role: Role, text: impl Into<String>) -> usize {
        self.messages.push(Message {
            role,
            text: text.into(),
            kind: MessageKind::Plain,
            file_path: None,
        });
        self.messages.len() - 1
    }

    fn append_diff_message(
        &mut self,
        role: Role,
        file_path: Option<String>,
        diff: impl Into<String>,
    ) -> usize {
        self.messages.push(Message {
            role,
            text: diff.into(),
            kind: MessageKind::Diff,
            file_path,
        });
        self.messages.len() - 1
    }

    fn put_agent_item_mapping(&mut self, item_id: &str, idx: usize) {
        self.agent_item_to_index.insert(item_id.to_string(), idx);
    }

    fn upsert_agent_delta(&mut self, item_id: &str, delta: &str) {
        if let Some(idx) = self.agent_item_to_index.get(item_id).copied() {
            if let Some(msg) = self.messages.get_mut(idx) {
                if msg.kind != MessageKind::Plain {
                    msg.kind = MessageKind::Plain;
                    msg.file_path = None;
                    msg.text.clear();
                }
                msg.text.push_str(delta);
            }
            return;
        }

        let idx = self.append_message(Role::Assistant, delta);
        self.put_agent_item_mapping(item_id, idx);
    }

    fn upsert_turn_diff(&mut self, turn_id: &str, diff: &str) {
        if diff.trim().is_empty() {
            return;
        }

        if let Some(idx) = self.turn_diff_to_index.get(turn_id).copied() {
            if let Some(msg) = self.messages.get_mut(idx) {
                if msg.text == diff && msg.kind == MessageKind::Diff {
                    return;
                }
                msg.role = Role::ToolOutput;
                msg.text = diff.to_string();
                msg.kind = MessageKind::Diff;
                msg.file_path = None;
                self.auto_follow_bottom = true;
                return;
            }
        }

        let idx = self.append_diff_message(Role::ToolOutput, None, diff.to_string());
        self.turn_diff_to_index.insert(turn_id.to_string(), idx);
        self.auto_follow_bottom = true;
    }
}

fn make_input_area() -> TextArea<'static> {
    TextArea::default()
}

struct AppServerClient {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<String>>>>,
    events_rx: mpsc::Receiver<String>,
    next_id: AtomicU64,
    reader_thread: Option<thread::JoinHandle<()>>,
}

impl AppServerClient {
    fn start() -> Result<Self> {
        let mut child = Command::new("codex")
            .arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("failed to spawn `codex app-server`")?;

        let stdin = child.stdin.take().context("missing child stdin")?;
        let stdout = child.stdout.take().context("missing child stdout")?;

        let (events_tx, events_rx) = mpsc::channel::<String>();
        let pending: Arc<Mutex<HashMap<u64, mpsc::Sender<String>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_for_thread = Arc::clone(&pending);

        let reader_thread = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
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

                let parsed: Value = match serde_json::from_str(trimmed) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                if let Some(method) = parsed.get("method").and_then(Value::as_str) {
                    if method.starts_with("codex/event/") {
                        continue;
                    }
                }

                if parsed.get("method").is_none() {
                    if let Some(id) = json_id_to_u64(parsed.get("id")) {
                        if let Some(tx) = pending_for_thread
                            .lock()
                            .ok()
                            .and_then(|mut p| p.remove(&id))
                        {
                            let _ = tx.send(trimmed.to_string());
                            continue;
                        }
                    }
                }

                let _ = events_tx.send(trimmed.to_string());
            }
        });

        Ok(Self {
            child,
            stdin: Arc::new(Mutex::new(stdin)),
            pending,
            events_rx,
            next_id: AtomicU64::new(1),
            reader_thread: Some(reader_thread),
        })
    }

    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel::<String>();

        self.pending
            .lock()
            .map_err(|_| anyhow!("pending lock poisoned"))?
            .insert(id, tx);

        let line = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        })
        .to_string();

        {
            let mut stdin = self
                .stdin
                .lock()
                .map_err(|_| anyhow!("stdin lock poisoned"))?;
            stdin.write_all(line.as_bytes())?;
            stdin.write_all(b"\n")?;
            stdin.flush()?;
        }

        match rx.recv_timeout(timeout) {
            Ok(resp) => Ok(resp),
            Err(_) => {
                let _ = self.pending.lock().map(|mut p| p.remove(&id));
                bail!("timeout waiting for {method}");
            }
        }
    }

    fn drain_events(&self, out: &mut Vec<String>) {
        while let Ok(line) = self.events_rx.try_recv() {
            out.push(line);
        }
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

impl Drop for AppServerClient {
    fn drop(&mut self) {
        self.stop();
    }
}

fn json_id_to_u64(v: Option<&Value>) -> Option<u64> {
    let v = v?;
    if let Some(n) = v.as_u64() {
        return Some(n);
    }
    if let Some(n) = v.as_i64() {
        return (n >= 0).then_some(n as u64);
    }
    if let Some(n) = v.as_f64() {
        return (n >= 0.0).then_some(n as u64);
    }
    v.as_str()?.parse::<u64>().ok()
}

fn params_initialize() -> Value {
    json!({
        "clientInfo": {
            "name": "carlos",
            "title": "carlos",
            "version": "0.1.0"
        },
        "capabilities": {
            "experimentalApi": true
        }
    })
}

fn params_thread_start(cwd: &str) -> Value {
    json!({
        "experimentalRawEvents": false,
        "persistExtendedHistory": true,
        "cwd": cwd,
    })
}

fn params_thread_resume(thread_id: &str) -> Value {
    json!({
        "threadId": thread_id,
        "persistExtendedHistory": true,
    })
}

fn params_thread_list(cwd: &str) -> Value {
    json!({
        "limit": 100,
        "cwd": cwd,
    })
}

fn params_turn_start(thread_id: &str, text: &str) -> Value {
    json!({
        "threadId": thread_id,
        "input": [{
            "type": "text",
            "text": text,
            "text_elements": []
        }]
    })
}

fn params_turn_steer(thread_id: &str, turn_id: &str, text: &str) -> Value {
    json!({
        "threadId": thread_id,
        "expectedTurnId": turn_id,
        "input": [{
            "type": "text",
            "text": text,
            "text_elements": []
        }]
    })
}

fn initialize_client(client: &AppServerClient) -> Result<()> {
    let resp = client.call("initialize", params_initialize(), Duration::from_secs(10))?;
    extract_result_object(&resp)?;
    Ok(())
}

fn extract_result_object(line: &str) -> Result<Value> {
    let parsed: Value = serde_json::from_str(line).context("invalid JSON response")?;
    if parsed.get("error").is_some() {
        bail!(
            "server returned error: {}",
            parsed.get("error").unwrap_or(&Value::Null)
        );
    }
    if parsed.get("result").is_none() {
        bail!("missing result in response");
    }
    Ok(parsed)
}

fn parse_thread_id_from_start_or_resume(response_line: &str) -> Result<String> {
    let parsed = extract_result_object(response_line)?;
    let result = parsed
        .get("result")
        .and_then(Value::as_object)
        .context("invalid result object")?;
    let thread = result
        .get("thread")
        .and_then(Value::as_object)
        .context("missing thread in result")?;
    let id = thread
        .get("id")
        .and_then(Value::as_str)
        .context("missing thread.id")?;
    Ok(id.to_string())
}

fn is_tool_call_type(kind: &str) -> bool {
    matches!(
        kind,
        "toolCall" | "tool_call" | "toolInvocation" | "functionCall" | "mcpToolCall"
    )
}

fn is_tool_output_type(kind: &str) -> bool {
    matches!(
        kind,
        "toolResult" | "toolOutput" | "tool_result" | "functionCallOutput" | "mcpToolResult"
    )
}

fn role_for_tool_type(kind: &str) -> Option<Role> {
    if kind == "commandExecution" {
        return Some(Role::ToolCall);
    }
    if is_tool_call_type(kind) {
        return Some(Role::ToolCall);
    }
    if is_tool_output_type(kind) {
        return Some(Role::ToolOutput);
    }
    None
}

fn is_probably_diff_text(text: &str) -> bool {
    let t = text.trim();
    t.starts_with("diff --git ")
        || t.starts_with("@@ ")
        || (t.contains("\n@@ ") && (t.contains("\n+++ ") || t.contains("\n--- ")))
        || (t.contains('\n') && t.contains("\n+++ ") && t.contains("\n--- "))
}

fn infer_file_path_from_object(obj: &serde_json::Map<String, Value>) -> Option<String> {
    for key in ["filePath", "path", "file", "filename"] {
        if let Some(v) = obj.get(key).and_then(Value::as_str) {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn collect_diff_blocks_recursive(
    value: &Value,
    current_file_path: Option<&str>,
    seen: &mut HashSet<String>,
    out: &mut Vec<DiffBlock>,
) {
    match value {
        Value::Object(obj) => {
            let inferred_path = infer_file_path_from_object(obj);
            let local_path = inferred_path.as_deref().or(current_file_path);

            if let Some(diff) = obj.get("diff").and_then(Value::as_str) {
                if !diff.is_empty() && is_probably_diff_text(diff) {
                    let key = format!("{}::{}", local_path.unwrap_or(""), diff);
                    if seen.insert(key) {
                        out.push(DiffBlock {
                            file_path: local_path.map(ToOwned::to_owned),
                            diff: diff.to_string(),
                        });
                    }
                }
            }

            for nested in obj.values() {
                collect_diff_blocks_recursive(nested, local_path, seen, out);
            }
        }
        Value::Array(arr) => {
            for nested in arr {
                collect_diff_blocks_recursive(nested, current_file_path, seen, out);
            }
        }
        _ => {}
    }
}

fn extract_diff_blocks(item: &Value) -> Vec<DiffBlock> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    collect_diff_blocks_recursive(item, None, &mut seen, &mut out);
    out
}

fn value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = value;
    for key in path {
        cur = cur.get(*key)?;
    }
    Some(cur)
}

fn first_string_at_paths(value: &Value, paths: &[&[&str]]) -> Option<String> {
    for path in paths {
        if let Some(s) = value_at_path(value, path).and_then(Value::as_str) {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn first_i64_at_paths(value: &Value, paths: &[&[&str]]) -> Option<i64> {
    for path in paths {
        if let Some(v) = value_at_path(value, path) {
            if let Some(n) = v.as_i64() {
                return Some(n);
            }
            if let Some(n) = v.as_u64() {
                return i64::try_from(n).ok();
            }
        }
    }
    None
}

fn command_execution_action_command(item: &Value) -> Option<String> {
    let actions = item.get("commandActions").and_then(Value::as_array)?;
    for a in actions {
        if let Some(cmd) = a.get("command").and_then(Value::as_str) {
            if !cmd.is_empty() {
                return Some(cmd.to_string());
            }
        }
    }
    None
}

fn normalize_shell_command(raw: &str) -> String {
    if let Some(pos) = raw.find(" -lc '") {
        let s = &raw[(pos + 6)..];
        if let Some(end) = s.rfind('\'') {
            return s[..end].to_string();
        }
    }
    raw.to_string()
}

fn tool_name(item: &Value) -> Option<String> {
    first_string_at_paths(
        item,
        &[
            &["tool"],
            &["name"],
            &["input", "tool"],
            &["input", "name"],
            &["function", "name"],
        ],
    )
}

fn tool_command(item: &Value) -> Option<String> {
    if item.get("type").and_then(Value::as_str) == Some("commandExecution") {
        if let Some(cmd) = command_execution_action_command(item) {
            return Some(cmd);
        }
        if let Some(raw) = item.get("command").and_then(Value::as_str) {
            if !raw.is_empty() {
                return Some(normalize_shell_command(raw));
            }
        }
    }

    first_string_at_paths(
        item,
        &[
            &["command"],
            &["input", "command"],
            &["action", "command"],
            &["args", "command"],
            &["arguments", "command"],
            &["metadata", "command"],
        ],
    )
}

fn tool_reasoning(item: &Value) -> Option<String> {
    first_string_at_paths(
        item,
        &[
            &["reasoning"],
            &["input", "reasoning"],
            &["metadata", "reasoning"],
            &["thought"],
        ],
    )
}

fn tool_description(item: &Value) -> Option<String> {
    first_string_at_paths(
        item,
        &[
            &["description"],
            &["input", "description"],
            &["metadata", "description"],
            &["title"],
            &["metadata", "title"],
        ],
    )
}

fn tool_output_text(item: &Value) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(s) = first_string_at_paths(item, &[&["aggregatedOutput"]]) {
        parts.push(s);
    }

    if let Some(s) = first_string_at_paths(
        item,
        &[
            &["output"],
            &["text"],
            &["result"],
            &["metadata", "output"],
            &["metadata", "result"],
            &["state", "output"],
            &["formattedOutput"],
        ],
    ) {
        parts.push(s);
    }
    if let Some(s) = first_string_at_paths(
        item,
        &[&["stdout"], &["metadata", "stdout"], &["state", "stdout"]],
    ) {
        parts.push(s);
    }
    if let Some(s) = first_string_at_paths(
        item,
        &[&["stderr"], &["metadata", "stderr"], &["state", "stderr"]],
    ) {
        parts.push(s);
    }
    if let Some(code) = first_i64_at_paths(
        item,
        &[
            &["exitCode"],
            &["exit_code"],
            &["metadata", "exitCode"],
            &["metadata", "exit_code"],
            &["state", "exitCode"],
            &["durationMs"],
        ],
    ) {
        if item.get("durationMs").is_some() && item.get("exitCode").is_none() {
            parts.push(format!("duration: {code} ms"));
        } else {
            parts.push(format!("exit code: {code}"));
        }
    }

    if let Some(code) = first_i64_at_paths(item, &[&["exitCode"], &["exit_code"]]) {
        if !parts.iter().any(|p| p.starts_with("exit code:")) {
            parts.push(format!("exit code: {code}"));
        }
    }

    parts.retain(|s| !s.trim().is_empty());
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn compact_json_summary(value: &Value, max_chars: usize) -> Option<String> {
    let mut s = serde_json::to_string(value).ok()?;
    if s.len() > max_chars {
        s.truncate(max_chars.saturating_sub(1));
        s.push('…');
    }
    Some(s)
}

fn tool_input_object<'a>(item: &'a Value) -> Option<&'a serde_json::Map<String, Value>> {
    value_at_path(item, &["input"]).and_then(Value::as_object)
}

fn inline_value(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => {
            if s.is_empty() {
                None
            } else {
                Some(s.clone())
            }
        }
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        Value::Array(_) | Value::Object(_) => compact_json_summary(value, 64),
    }
}

fn format_input_brackets(
    input: &serde_json::Map<String, Value>,
    skip_keys: &[&str],
) -> Option<String> {
    let skip: HashSet<&str> = skip_keys.iter().copied().collect();
    let mut keys: Vec<&str> = input
        .keys()
        .map(String::as_str)
        .filter(|k| !skip.contains(*k))
        .collect();
    keys.sort_unstable();

    let mut parts = Vec::new();
    for key in keys {
        let Some(val) = input.get(key).and_then(inline_value) else {
            continue;
        };
        if val.is_empty() {
            continue;
        }
        parts.push(format!("{key}={val}"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(format!("[{}]", parts.join(" ")))
    }
}

fn titlecase_tool_name(name: &str) -> String {
    let normalized = name.replace(['_', '-'], " ");
    let mut out = String::new();
    for (i, word) in normalized.split_whitespace().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(chars.as_str());
        }
    }
    if out.is_empty() {
        name.to_string()
    } else {
        out
    }
}

fn tool_icon(tool_name: &str) -> &'static str {
    match tool_name {
        "read" | "list" => "→",
        "write" | "edit" | "applypatch" | "apply_patch" => "←",
        "grep" | "glob" | "codesearch" => "✱",
        "task" => "#",
        _ => "◇",
    }
}

fn format_tool_call_inline(item: &Value, tool_name: &str) -> Option<String> {
    let lower = tool_name.to_ascii_lowercase();
    let input = tool_input_object(item);

    match lower.as_str() {
        "read" => {
            let path = input
                .and_then(|obj| {
                    obj.get("filePath")
                        .and_then(Value::as_str)
                        .or_else(|| obj.get("path").and_then(Value::as_str))
                })
                .unwrap_or("");
            let mut out = format!("{} Read {}", tool_icon(&lower), path);
            if let Some(args) = input.and_then(|obj| {
                format_input_brackets(
                    obj,
                    &[
                        "filePath",
                        "path",
                        "tool",
                        "name",
                        "command",
                        "description",
                        "reasoning",
                    ],
                )
            }) {
                if !args.is_empty() {
                    out.push(' ');
                    out.push_str(&args);
                }
            }
            return Some(out.trim_end().to_string());
        }
        _ => {}
    }

    if let Some(obj) = input {
        let mut out = format!("{} {}", tool_icon(&lower), titlecase_tool_name(tool_name));
        if let Some(path) = obj
            .get("filePath")
            .and_then(Value::as_str)
            .or_else(|| obj.get("path").and_then(Value::as_str))
        {
            if !path.is_empty() {
                out.push(' ');
                out.push_str(path);
            }
        }

        if let Some(args) = format_input_brackets(
            obj,
            &[
                "filePath",
                "path",
                "tool",
                "name",
                "command",
                "description",
                "reasoning",
                "content",
                "patch",
                "old_string",
                "new_string",
                "text",
            ],
        ) {
            out.push(' ');
            out.push_str(&args);
        }
        return Some(out);
    }

    Some(format!(
        "{} {}",
        tool_icon(&lower),
        titlecase_tool_name(tool_name)
    ))
}

fn tool_input_summary(item: &Value) -> Option<String> {
    if let Some(obj) = value_at_path(item, &["input"]).and_then(Value::as_object) {
        let mut fields = Vec::new();
        for key in [
            "filePath",
            "path",
            "pattern",
            "query",
            "url",
            "method",
            "subagent_type",
        ] {
            if let Some(v) = obj.get(key).and_then(Value::as_str) {
                if !v.is_empty() {
                    fields.push(format!("{key}={v}"));
                }
            }
        }
        if !fields.is_empty() {
            return Some(fields.join(" "));
        }
        return compact_json_summary(&Value::Object(obj.clone()), 220);
    }
    None
}

fn format_tool_item(item: &Value, role: Role) -> Option<String> {
    match role {
        Role::ToolCall => {
            if let Some(cmd) = tool_command(item) {
                let mut lines = Vec::new();
                lines.push(format!("run `{cmd}`"));
                if let Some(reason) = tool_reasoning(item) {
                    lines.push(format!("Thinking: {reason}"));
                }
                if let Some(desc) = tool_description(item) {
                    lines.push(format!("# {desc}"));
                }
                lines.push(format!("$ {cmd}"));
                return Some(lines.join("\n"));
            }

            if let Some(name) = tool_name(item) {
                if let Some(inline) = format_tool_call_inline(item, &name) {
                    return Some(inline);
                }
                if let Some(input) = tool_input_summary(item) {
                    return Some(format!("{} {}", titlecase_tool_name(&name), input));
                }
                return Some(titlecase_tool_name(&name));
            }

            None
        }
        Role::ToolOutput => {
            if let Some(out) = tool_output_text(item) {
                return Some(out);
            }
            None
        }
        _ => None,
    }
}

fn collect_text_parts(content: &[Value]) -> Vec<&str> {
    let mut text_parts = Vec::new();
    for part in content {
        if part.get("type").and_then(Value::as_str) != Some("text") {
            continue;
        }
        if let Some(t) = part.get("text").and_then(Value::as_str) {
            text_parts.push(t);
        }
    }
    text_parts
}

fn append_item_text_from_content(app: &mut AppState, item: &Value, role: Role) {
    let Some(content) = item.get("content").and_then(Value::as_array) else {
        return;
    };
    let text_parts = collect_text_parts(content);
    if !text_parts.is_empty() {
        app.append_message(role, text_parts.join("\n"));
    }
}

fn append_tool_history_item(app: &mut AppState, item: &Value, role: Role) {
    let diffs = extract_diff_blocks(item);
    if !diffs.is_empty() {
        for block in diffs {
            app.append_diff_message(role, block.file_path, block.diff);
        }
        return;
    }

    if let Some(formatted) = format_tool_item(item, role) {
        if !formatted.is_empty() {
            app.append_message(role, formatted);
            return;
        }
    }

    if let Some(t) = item.get("text").and_then(Value::as_str) {
        if !t.is_empty() {
            app.append_message(role, t.to_string());
            return;
        }
    }
    append_item_text_from_content(app, item, role);
}

fn append_history_from_thread(app: &mut AppState, thread_obj: &Value) {
    let Some(turns) = thread_obj.get("turns").and_then(Value::as_array) else {
        return;
    };

    for turn in turns {
        let Some(items) = turn.get("items").and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let Some(kind) = item.get("type").and_then(Value::as_str) else {
                continue;
            };

            match kind {
                "userMessage" => {
                    append_item_text_from_content(app, item, Role::User);
                }
                "agentMessage" => {
                    if let Some(t) = item.get("text").and_then(Value::as_str) {
                        app.append_message(Role::Assistant, t.to_string());
                    }
                }
                "reasoning" => {
                    let Some(summary) = item.get("summary").and_then(Value::as_array) else {
                        continue;
                    };
                    let mut parts = Vec::new();
                    for s in summary {
                        if let Some(t) = s.as_str() {
                            parts.push(t);
                        }
                    }
                    if !parts.is_empty() {
                        app.append_message(Role::Reasoning, parts.join("\n"));
                    }
                }
                "commandExecution" => {
                    append_tool_history_item(app, item, Role::ToolOutput);
                }
                k if is_tool_call_type(k) => {
                    append_tool_history_item(app, item, Role::ToolCall);
                }
                k if is_tool_output_type(k) => {
                    append_tool_history_item(app, item, Role::ToolOutput);
                }
                _ => {}
            }
        }
    }
}

fn load_history_from_start_or_resume(app: &mut AppState, response_line: &str) -> Result<()> {
    let parsed = extract_result_object(response_line)?;
    if let Some(thread_obj) = parsed
        .get("result")
        .and_then(Value::as_object)
        .and_then(|r| r.get("thread"))
    {
        append_history_from_thread(app, thread_obj);
    }
    Ok(())
}

fn parse_thread_list(response_line: &str) -> Result<Vec<ThreadSummary>> {
    let parsed = extract_result_object(response_line)?;
    let Some(data) = parsed
        .get("result")
        .and_then(Value::as_object)
        .and_then(|r| r.get("data"))
        .and_then(Value::as_array)
    else {
        return Ok(Vec::new());
    };

    let mut out = Vec::new();
    for entry in data {
        let Some(obj) = entry.as_object() else {
            continue;
        };
        let Some(id) = obj.get("id").and_then(Value::as_str) else {
            continue;
        };
        let preview = obj.get("preview").and_then(Value::as_str).unwrap_or("");
        let cwd = obj.get("cwd").and_then(Value::as_str).unwrap_or("");
        let updated_at = obj
            .get("updatedAt")
            .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64)))
            .unwrap_or(0);

        out.push(ThreadSummary {
            id: id.to_string(),
            preview: preview.to_string(),
            cwd: cwd.to_string(),
            updated_at,
        });
    }
    Ok(out)
}

fn visual_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

fn split_at_cells(s: &str, max_cells: usize) -> usize {
    if max_cells == 0 || s.is_empty() {
        return 0;
    }

    let mut cells = 0usize;
    let mut idx = 0usize;

    for (byte_idx, g) in s.grapheme_indices(true) {
        let w = visual_width(g);
        if w > 0 && cells + w > max_cells {
            break;
        }
        cells += w;
        idx = byte_idx + g.len();
    }

    if idx == 0 {
        if let Some(g) = s.graphemes(true).next() {
            return g.len();
        }
    }

    idx
}

fn slice_by_cells(s: &str, start: usize, end: usize) -> String {
    if start >= end || s.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    let mut col = 0usize;

    for g in s.graphemes(true) {
        let w = visual_width(g);
        if w == 0 {
            if col >= start && col < end {
                out.push_str(g);
            }
            continue;
        }

        let next = col + w;
        if next <= start {
            col = next;
            continue;
        }
        if col >= end {
            break;
        }

        out.push_str(g);
        col = next;
    }

    out
}

fn role_prefix(role: Role) -> &'static str {
    match role {
        Role::User => "",
        Role::Assistant => "",
        Role::Reasoning => "",
        Role::ToolCall => "",
        Role::ToolOutput => "",
        Role::System => "",
    }
}

#[derive(Debug, Clone, Copy)]
struct CarlosMarkdownStyleSheet;

fn color_to_core(color: Color) -> CoreColor {
    match color {
        Color::Reset => CoreColor::Reset,
        Color::Black => CoreColor::Black,
        Color::Red => CoreColor::Red,
        Color::Green => CoreColor::Green,
        Color::Yellow => CoreColor::Yellow,
        Color::Blue => CoreColor::Blue,
        Color::Magenta => CoreColor::Magenta,
        Color::Cyan => CoreColor::Cyan,
        Color::Gray => CoreColor::Gray,
        Color::DarkGray => CoreColor::DarkGray,
        Color::LightRed => CoreColor::LightRed,
        Color::LightGreen => CoreColor::LightGreen,
        Color::LightYellow => CoreColor::LightYellow,
        Color::LightBlue => CoreColor::LightBlue,
        Color::LightMagenta => CoreColor::LightMagenta,
        Color::LightCyan => CoreColor::LightCyan,
        Color::White => CoreColor::White,
        Color::Rgb(r, g, b) => CoreColor::Rgb(r, g, b),
        Color::Indexed(v) => CoreColor::Indexed(v),
    }
}

fn core_color_to_color(color: CoreColor) -> Color {
    match color {
        CoreColor::Reset => Color::Reset,
        CoreColor::Black => Color::Black,
        CoreColor::Red => Color::Red,
        CoreColor::Green => Color::Green,
        CoreColor::Yellow => Color::Yellow,
        CoreColor::Blue => Color::Blue,
        CoreColor::Magenta => Color::Magenta,
        CoreColor::Cyan => Color::Cyan,
        CoreColor::Gray => Color::Gray,
        CoreColor::DarkGray => Color::DarkGray,
        CoreColor::LightRed => Color::LightRed,
        CoreColor::LightGreen => Color::LightGreen,
        CoreColor::LightYellow => Color::LightYellow,
        CoreColor::LightBlue => Color::LightBlue,
        CoreColor::LightMagenta => Color::LightMagenta,
        CoreColor::LightCyan => Color::LightCyan,
        CoreColor::White => Color::White,
        CoreColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
        CoreColor::Indexed(v) => Color::Indexed(v),
    }
}

fn modifier_to_core(modifier: Modifier) -> CoreModifier {
    let mut out = CoreModifier::empty();
    if modifier.contains(Modifier::BOLD) {
        out |= CoreModifier::BOLD;
    }
    if modifier.contains(Modifier::DIM) {
        out |= CoreModifier::DIM;
    }
    if modifier.contains(Modifier::ITALIC) {
        out |= CoreModifier::ITALIC;
    }
    if modifier.contains(Modifier::UNDERLINED) {
        out |= CoreModifier::UNDERLINED;
    }
    if modifier.contains(Modifier::SLOW_BLINK) {
        out |= CoreModifier::SLOW_BLINK;
    }
    if modifier.contains(Modifier::RAPID_BLINK) {
        out |= CoreModifier::RAPID_BLINK;
    }
    if modifier.contains(Modifier::REVERSED) {
        out |= CoreModifier::REVERSED;
    }
    if modifier.contains(Modifier::HIDDEN) {
        out |= CoreModifier::HIDDEN;
    }
    if modifier.contains(Modifier::CROSSED_OUT) {
        out |= CoreModifier::CROSSED_OUT;
    }
    out
}

fn core_modifier_to_modifier(modifier: CoreModifier) -> Modifier {
    let mut out = Modifier::empty();
    if modifier.contains(CoreModifier::BOLD) {
        out |= Modifier::BOLD;
    }
    if modifier.contains(CoreModifier::DIM) {
        out |= Modifier::DIM;
    }
    if modifier.contains(CoreModifier::ITALIC) {
        out |= Modifier::ITALIC;
    }
    if modifier.contains(CoreModifier::UNDERLINED) {
        out |= Modifier::UNDERLINED;
    }
    if modifier.contains(CoreModifier::SLOW_BLINK) {
        out |= Modifier::SLOW_BLINK;
    }
    if modifier.contains(CoreModifier::RAPID_BLINK) {
        out |= Modifier::RAPID_BLINK;
    }
    if modifier.contains(CoreModifier::REVERSED) {
        out |= Modifier::REVERSED;
    }
    if modifier.contains(CoreModifier::HIDDEN) {
        out |= Modifier::HIDDEN;
    }
    if modifier.contains(CoreModifier::CROSSED_OUT) {
        out |= Modifier::CROSSED_OUT;
    }
    out
}

fn style_to_core(style: Style) -> CoreStyle {
    let mut out = CoreStyle::default();
    if let Some(fg) = style.fg {
        out = out.fg(color_to_core(fg));
    }
    if let Some(bg) = style.bg {
        out = out.bg(color_to_core(bg));
    }
    if let Some(ul) = style.underline_color {
        out = out.underline_color(color_to_core(ul));
    }
    out.add_modifier = modifier_to_core(style.add_modifier);
    out.sub_modifier = modifier_to_core(style.sub_modifier);
    out
}

fn core_style_to_style(style: CoreStyle) -> Style {
    let mut out = Style::default();
    if let Some(fg) = style.fg {
        out = out.fg(core_color_to_color(fg));
    }
    if let Some(bg) = style.bg {
        out = out.bg(core_color_to_color(bg));
    }
    if let Some(ul) = style.underline_color {
        out = out.underline_color(core_color_to_color(ul));
    }
    out.add_modifier = core_modifier_to_modifier(style.add_modifier);
    out.sub_modifier = core_modifier_to_modifier(style.sub_modifier);
    out
}

impl tui_markdown::StyleSheet for CarlosMarkdownStyleSheet {
    fn heading(&self, level: u8) -> CoreStyle {
        match level {
            1 => style_to_core(
                Style::default()
                    .fg(COLOR_TEXT)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ),
            2 => style_to_core(Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD)),
            _ => style_to_core(Style::default().fg(COLOR_TEXT)),
        }
    }

    fn code(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_TEXT))
    }

    fn link(&self) -> CoreStyle {
        style_to_core(
            Style::default()
                .fg(COLOR_GUTTER_USER)
                .add_modifier(Modifier::UNDERLINED),
        )
    }

    fn blockquote(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_DIM))
    }

    fn heading_meta(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_DIM).add_modifier(Modifier::DIM))
    }

    fn metadata_block(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_DIM))
    }
}

fn is_fence_delimiter(line: &str) -> bool {
    line.trim_matches([' ', '\t', '\r']).starts_with("```")
}

fn styled_plain_text(segments: &[StyledSegment]) -> String {
    let mut out = String::new();
    for seg in segments {
        out.push_str(&seg.text);
    }
    out
}

fn markdown_line_segments(text: &str) -> Vec<Vec<StyledSegment>> {
    let opts = MarkdownOptions::new(CarlosMarkdownStyleSheet);
    let markdown = markdown_from_str_with_options(text, &opts);

    let mut out = Vec::new();
    for line in markdown.lines {
        let mut segments = Vec::new();
        for span in line.spans {
            if span.content.is_empty() {
                continue;
            }
            segments.push(StyledSegment {
                text: span.content.to_string(),
                style: core_style_to_style(span.style),
            });
        }

        let plain = styled_plain_text(&segments);
        if is_fence_delimiter(&plain) {
            continue;
        }
        out.push(segments);
    }
    out
}

fn take_styled_segments_by_cells(
    remaining: &mut VecDeque<StyledSegment>,
    max_cells: usize,
) -> Vec<StyledSegment> {
    if max_cells == 0 {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut taken_cells = 0usize;

    while let Some(mut seg) = remaining.pop_front() {
        let seg_cells = visual_width(&seg.text);
        if seg_cells == 0 {
            out.push(seg);
            continue;
        }

        if taken_cells + seg_cells <= max_cells {
            taken_cells += seg_cells;
            out.push(seg);
            if taken_cells == max_cells {
                break;
            }
            continue;
        }

        let allowed = max_cells.saturating_sub(taken_cells);
        if allowed == 0 {
            remaining.push_front(seg);
            break;
        }

        let split = split_at_cells(&seg.text, allowed);
        if split == 0 {
            remaining.push_front(seg);
            break;
        }

        let left = seg.text[..split].to_string();
        let right = seg.text[split..].to_string();
        let seg_style = seg.style;
        if !right.is_empty() {
            seg.text = right;
            remaining.push_front(seg);
        }

        out.push(StyledSegment {
            text: left,
            style: seg_style,
        });
        break;
    }

    out
}

fn wrap_natural_by_cells(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    if text.is_empty() {
        return vec![String::new()];
    }

    let options = WrapOptions::new(width)
        .break_words(false)
        .word_splitter(WordSplitter::NoHyphenation);
    let wrapped = wrap_text(text, options);
    if wrapped.is_empty() {
        return vec![String::new()];
    }

    let mut out = Vec::new();
    for piece in wrapped {
        let s = piece.into_owned();
        if visual_width(&s) <= width {
            out.push(s);
            continue;
        }

        // Extremely long tokens can still overflow when word breaking is disabled.
        // Fall back to hard cell wrapping only in that case.
        let mut rest = s.as_str();
        loop {
            let take = split_at_cells(rest, width);
            if take == 0 {
                out.push(rest.to_string());
                break;
            }
            out.push(rest[..take].to_string());
            if take >= rest.len() {
                break;
            }
            rest = &rest[take..];
        }
    }

    out
}

fn append_wrapped_message_lines(out: &mut Vec<RenderedLine>, role: Role, text: &str, width: usize) {
    if width < 8 {
        return;
    }

    let prefix = role_prefix(role);
    let mut first_physical = true;
    let mut in_code_fence = false;

    for logical in text.split('\n') {
        if is_fence_delimiter(logical) {
            in_code_fence = !in_code_fence;
            continue;
        }

        let continuation = match role {
            Role::Reasoning => "          ",
            Role::ToolCall => "      ",
            Role::ToolOutput => "        ",
            _ => role_prefix(role),
        };

        if logical.is_empty() {
            let t = prefix.to_string();
            out.push(RenderedLine {
                cells: visual_width(&t),
                text: t,
                styled_segments: Vec::new(),
                role,
                separator: false,
                soft_wrap_to_next: false,
            });
            first_physical = false;
            continue;
        }

        let lead_for_width = if in_code_fence {
            role_prefix(Role::System)
        } else if first_physical {
            prefix
        } else {
            continuation
        };
        let lead_cells = visual_width(lead_for_width);
        let avail = width.saturating_sub(lead_cells);
        if avail == 0 {
            first_physical = false;
            continue;
        }
        let wrapped_parts = wrap_natural_by_cells(logical, avail);

        for (i, part) in wrapped_parts.iter().enumerate() {
            let lead = if in_code_fence {
                role_prefix(Role::System)
            } else if first_physical && i == 0 {
                prefix
            } else {
                continuation
            };

            let mut line = String::with_capacity(lead.len() + part.len());
            line.push_str(lead);
            line.push_str(part);
            let wrapped = i + 1 < wrapped_parts.len();

            out.push(RenderedLine {
                cells: visual_width(&line),
                text: line.clone(),
                styled_segments: vec![StyledSegment {
                    text: line,
                    style: Style::default(),
                }],
                role,
                separator: false,
                soft_wrap_to_next: wrapped,
            });
        }

        first_physical = false;
    }
}

fn append_wrapped_markdown_lines(
    out: &mut Vec<RenderedLine>,
    role: Role,
    text: &str,
    width: usize,
) {
    if width < 8 {
        return;
    }

    let logical_lines = markdown_line_segments(text);
    for logical in logical_lines {
        let plain = styled_plain_text(&logical);
        if plain.is_empty() {
            out.push(RenderedLine {
                cells: 0,
                text: String::new(),
                styled_segments: Vec::new(),
                role,
                separator: false,
                soft_wrap_to_next: false,
            });
            continue;
        }

        let wrapped_parts = wrap_natural_by_cells(&plain, width);
        let mut remaining: VecDeque<StyledSegment> = logical.into();

        for (i, part) in wrapped_parts.iter().enumerate() {
            let part_cells = visual_width(part);
            let wrapped = i + 1 < wrapped_parts.len();
            let styled_segments = take_styled_segments_by_cells(&mut remaining, part_cells);

            out.push(RenderedLine {
                cells: part_cells,
                text: part.clone(),
                styled_segments,
                role,
                separator: false,
                soft_wrap_to_next: wrapped,
            });
        }
    }
}

fn diff_line_style(line: &str) -> Style {
    if line.starts_with("@@") {
        return Style::default()
            .fg(COLOR_DIFF_HUNK)
            .add_modifier(Modifier::BOLD);
    }
    if (line.starts_with("+++") || line.starts_with("---")) && line.len() > 3 {
        return Style::default().fg(COLOR_DIFF_HEADER);
    }
    if line.starts_with("diff --git ")
        || line.starts_with("index ")
        || line.starts_with("new file mode ")
        || line.starts_with("deleted file mode ")
    {
        return Style::default()
            .fg(COLOR_DIFF_HEADER)
            .add_modifier(Modifier::DIM);
    }
    if line.starts_with('+') && !line.starts_with("+++") {
        return Style::default().fg(COLOR_DIFF_ADD);
    }
    if line.starts_with('-') && !line.starts_with("---") {
        return Style::default().fg(COLOR_DIFF_REMOVE);
    }
    Style::default().fg(COLOR_TEXT)
}

fn append_wrapped_diff_lines(
    out: &mut Vec<RenderedLine>,
    role: Role,
    file_path: Option<&str>,
    diff: &str,
    width: usize,
) {
    if width < 8 {
        return;
    }

    if let Some(path) = file_path {
        if !path.is_empty() {
            out.push(RenderedLine {
                cells: visual_width(path),
                text: path.to_string(),
                styled_segments: vec![StyledSegment {
                    text: path.to_string(),
                    style: Style::default().fg(COLOR_DIM).add_modifier(Modifier::BOLD),
                }],
                role,
                separator: false,
                soft_wrap_to_next: false,
            });
        }
    }

    for logical in diff.split('\n') {
        let line_style = diff_line_style(logical);
        if logical.is_empty() {
            out.push(RenderedLine {
                cells: 0,
                text: String::new(),
                styled_segments: Vec::new(),
                role,
                separator: false,
                soft_wrap_to_next: false,
            });
            continue;
        }

        let wrapped_parts = wrap_natural_by_cells(logical, width);
        for (i, part) in wrapped_parts.iter().enumerate() {
            let wrapped = i + 1 < wrapped_parts.len();
            out.push(RenderedLine {
                cells: visual_width(part),
                text: part.clone(),
                styled_segments: vec![StyledSegment {
                    text: part.clone(),
                    style: line_style,
                }],
                role,
                separator: false,
                soft_wrap_to_next: wrapped,
            });
        }
    }
}

fn build_rendered_lines(messages: &[Message], width: usize) -> Vec<RenderedLine> {
    let mut out = Vec::new();

    for (idx, msg) in messages.iter().enumerate() {
        if idx > 0 {
            out.push(RenderedLine {
                text: String::new(),
                styled_segments: Vec::new(),
                role: Role::System,
                separator: true,
                cells: 0,
                soft_wrap_to_next: false,
            });
        }
        match msg.kind {
            MessageKind::Diff => append_wrapped_diff_lines(
                &mut out,
                msg.role,
                msg.file_path.as_deref(),
                &msg.text,
                width,
            ),
            MessageKind::Plain => match msg.role {
                Role::Assistant | Role::Reasoning => {
                    append_wrapped_markdown_lines(&mut out, msg.role, &msg.text, width);
                }
                _ => append_wrapped_message_lines(&mut out, msg.role, &msg.text, width),
            },
        }
    }

    out
}

fn transcript_content_width(size: TerminalSize) -> usize {
    if size.width > MSG_CONTENT_X + 1 {
        size.width - (MSG_CONTENT_X + 1)
    } else {
        0
    }
}

#[derive(Debug, Clone)]
struct InputLayout {
    msg_bottom: usize,   // 1-based; 0 means no transcript row is available
    input_top: usize,    // 1-based
    input_height: usize, // rows
    text_width: usize,   // cells available for input text
    visible_lines: Vec<String>,
    cursor_x: usize, // 0-based terminal column
    cursor_y: usize, // 0-based terminal row
}

fn char_to_byte_idx(s: &str, char_idx: usize) -> usize {
    if char_idx == 0 {
        return 0;
    }
    match s.char_indices().nth(char_idx) {
        Some((i, _)) => i,
        None => s.len(),
    }
}

fn wrap_line_cells(line: &str, width: usize, out: &mut Vec<String>) {
    if width == 0 {
        out.push(String::new());
        return;
    }
    if line.is_empty() {
        out.push(String::new());
        return;
    }
    let mut rest = line;
    loop {
        let take = split_at_cells(rest, width);
        if take == 0 {
            out.push(rest.to_string());
            break;
        }
        out.push(rest[..take].to_string());
        if take >= rest.len() {
            break;
        }
        rest = &rest[take..];
    }
}

fn wrapped_line_count(line: &str, width: usize) -> usize {
    if width == 0 || line.is_empty() {
        return 1;
    }
    let mut rest = line;
    let mut count = 0usize;
    loop {
        let take = split_at_cells(rest, width);
        if take == 0 {
            count += 1;
            break;
        }
        count += 1;
        if take >= rest.len() {
            break;
        }
        rest = &rest[take..];
    }
    count.max(1)
}

fn textarea_input_from_key(k: crossterm::event::KeyEvent) -> TextInput {
    let key = match k.code {
        KeyCode::Char(c) => TextKey::Char(c),
        KeyCode::Backspace => TextKey::Backspace,
        KeyCode::Enter => TextKey::Enter,
        KeyCode::Left => TextKey::Left,
        KeyCode::Right => TextKey::Right,
        KeyCode::Up => TextKey::Up,
        KeyCode::Down => TextKey::Down,
        KeyCode::Tab => TextKey::Tab,
        KeyCode::Delete => TextKey::Delete,
        KeyCode::Home => TextKey::Home,
        KeyCode::End => TextKey::End,
        KeyCode::PageUp => TextKey::PageUp,
        KeyCode::PageDown => TextKey::PageDown,
        KeyCode::Esc => TextKey::Esc,
        KeyCode::F(n) => TextKey::F(n),
        _ => TextKey::Null,
    };

    TextInput {
        key,
        ctrl: k.modifiers.contains(KeyModifiers::CONTROL),
        alt: k.modifiers.contains(KeyModifiers::ALT),
        shift: k.modifiers.contains(KeyModifiers::SHIFT),
    }
}

fn compute_input_layout(app: &AppState, size: TerminalSize) -> InputLayout {
    let text_width = transcript_content_width(size);

    let mut wrapped = Vec::new();
    for line in app.input.lines() {
        wrap_line_cells(line, text_width, &mut wrapped);
    }
    if wrapped.is_empty() {
        wrapped.push(String::new());
    }

    let mut max_input_rows = 8usize.min(size.height.max(1));
    if size.height > 1 {
        max_input_rows = max_input_rows.min(size.height - 1);
    }
    let input_height = wrapped.len().clamp(1, max_input_rows.max(1));

    let (cursor_row, cursor_col_chars) = app.input.cursor();
    let mut cursor_wrapped_row = 0usize;
    for line in app.input.lines().iter().take(cursor_row) {
        cursor_wrapped_row += wrapped_line_count(line, text_width);
    }
    let cursor_line = app
        .input
        .lines()
        .get(cursor_row)
        .map(String::as_str)
        .unwrap_or("");
    let cursor_byte = char_to_byte_idx(cursor_line, cursor_col_chars);
    let cursor_cells = visual_width(&cursor_line[..cursor_byte.min(cursor_line.len())]);
    let cursor_wrapped_col = if text_width == 0 {
        0
    } else {
        cursor_wrapped_row += cursor_cells / text_width;
        cursor_cells % text_width
    };

    let mut visible_start = wrapped.len().saturating_sub(input_height);
    if cursor_wrapped_row < visible_start {
        visible_start = cursor_wrapped_row;
    }
    if cursor_wrapped_row >= visible_start + input_height {
        visible_start = cursor_wrapped_row + 1 - input_height;
    }

    let visible_end = (visible_start + input_height).min(wrapped.len());
    let mut visible_lines = wrapped[visible_start..visible_end].to_vec();
    while visible_lines.len() < input_height {
        visible_lines.insert(0, String::new());
    }

    let input_top = size.height + 1 - input_height;
    let msg_bottom = input_top.saturating_sub(2);
    let cursor_visual_row = cursor_wrapped_row.saturating_sub(visible_start);
    let cursor_x = MSG_CONTENT_X + cursor_wrapped_col;
    let cursor_y = input_top.saturating_sub(1) + cursor_visual_row.min(input_height - 1);

    InputLayout {
        msg_bottom,
        input_top,
        input_height,
        text_width,
        visible_lines,
        cursor_x,
        cursor_y,
    }
}

fn compute_selection_range(
    selection: Selection,
    row: usize,
    line_cells: usize,
) -> Option<(usize, usize)> {
    let mut ax = selection.anchor_x;
    let mut ay = selection.anchor_y;
    let mut fx = selection.focus_x;
    let mut fy = selection.focus_y;

    if fy < ay || (fy == ay && fx < ax) {
        std::mem::swap(&mut ax, &mut fx);
        std::mem::swap(&mut ay, &mut fy);
    }

    if row < ay || row > fy {
        return None;
    }

    let mut start_col = 1usize;
    let mut end_col = line_cells;

    if row == ay {
        start_col = ax;
    }
    if row == fy {
        end_col = fx;
    }

    if start_col > end_col {
        return None;
    }

    if line_cells == 0 {
        return None;
    }

    start_col = start_col.max(1);
    end_col = end_col.min(line_cells);
    if start_col > line_cells {
        return None;
    }

    Some((start_col - 1, end_col))
}

fn selected_text(
    selection: Selection,
    rendered_lines: &[RenderedLine],
    msg_bottom: usize,
    scroll_top: usize,
) -> String {
    let msg_top = MSG_TOP;

    let mut ax = selection.anchor_x;
    let mut ay = selection.anchor_y;
    let mut fx = selection.focus_x;
    let mut fy = selection.focus_y;
    if fy < ay || (fy == ay && fx < ax) {
        std::mem::swap(&mut ax, &mut fx);
        std::mem::swap(&mut ay, &mut fy);
    }

    if msg_bottom < msg_top || fy < msg_top || ay > msg_bottom {
        return String::new();
    }

    let start_row = ay.max(msg_top);
    let end_row = fy.min(msg_bottom);

    let mut out = String::new();
    let mut first = true;
    let mut prev_row: Option<usize> = None;
    let mut prev_idx: Option<usize> = None;
    let mut prev_soft_wrap = false;

    for row in start_row..=end_row {
        let idx = scroll_top + (row - msg_top);
        if idx >= rendered_lines.len() {
            continue;
        }

        let line = &rendered_lines[idx];
        let line_cells = line.cells;

        let mut s_col = 1usize;
        let mut e_col = line_cells;
        if row == ay {
            s_col = ax;
        }
        if row == fy {
            e_col = fx;
        }

        s_col = s_col.max(1);
        e_col = e_col.min(line_cells);

        if !first {
            let contiguous =
                prev_row.is_some_and(|r| r + 1 == row) && prev_idx.is_some_and(|i| i + 1 == idx);
            if !(contiguous && prev_soft_wrap) {
                out.push('\n');
            }
        }
        first = false;

        if line_cells > 0 && s_col <= e_col && s_col <= line_cells {
            out.push_str(&slice_by_cells(&line.text, s_col - 1, e_col));
        }

        prev_row = Some(row);
        prev_idx = Some(idx);
        prev_soft_wrap = line.soft_wrap_to_next;
    }

    out
}

fn last_assistant_message(messages: &[Message]) -> Option<&str> {
    messages
        .iter()
        .rev()
        .find(|m| m.role == Role::Assistant && !m.text.is_empty())
        .map(|m| m.text.as_str())
}

fn copy_via_program(argv: &[&str], text: &str) -> bool {
    let Some((cmd, args)) = argv.split_first() else {
        return false;
    };

    let mut child = match Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }

    child.wait().map(|s| s.success()).unwrap_or(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Osc52Wrap {
    None,
    Tmux,
    Screen,
}

fn detect_osc52_wrap(tmux: Option<&str>, term: Option<&str>) -> Osc52Wrap {
    if tmux.is_some_and(|v| !v.is_empty()) {
        return Osc52Wrap::Tmux;
    }
    if term.is_some_and(|t| t.contains("screen")) {
        return Osc52Wrap::Screen;
    }
    Osc52Wrap::None
}

fn osc52_base_sequence(target: &str, encoded: &str, use_st_terminator: bool) -> String {
    if use_st_terminator {
        format!("\x1b]52;{};{}\x1b\\", target, encoded)
    } else {
        format!("\x1b]52;{};{}\x07", target, encoded)
    }
}

fn wrap_osc52_sequence(seq: &str, wrap: Osc52Wrap) -> String {
    match wrap {
        Osc52Wrap::None => seq.to_string(),
        Osc52Wrap::Tmux => {
            // tmux passthrough requires DCS wrapper and escaping nested ESC bytes.
            let escaped = seq.replace('\x1b', "\x1b\x1b");
            format!("\x1bPtmux;{}\x1b\\", escaped)
        }
        Osc52Wrap::Screen => {
            // GNU screen passthrough wrapper.
            format!("\x1bP{}\x1b\\", seq)
        }
    }
}

fn osc52_sequences_for_env(encoded: &str, tmux: Option<&str>, term: Option<&str>) -> Vec<String> {
    let wrap = detect_osc52_wrap(tmux, term);
    let mut out = Vec::with_capacity(4);
    for target in ["c", "p"] {
        out.push(wrap_osc52_sequence(
            &osc52_base_sequence(target, encoded, false),
            wrap,
        ));
        out.push(wrap_osc52_sequence(
            &osc52_base_sequence(target, encoded, true),
            wrap,
        ));
    }
    out
}

fn copy_via_osc52(text: &str) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    let tmux = env::var("TMUX").ok();
    let term = env::var("TERM").ok();
    let sequences = osc52_sequences_for_env(&encoded, tmux.as_deref(), term.as_deref());

    let mut stdout = io::stdout();
    for seq in &sequences {
        let _ = stdout.write_all(seq.as_bytes());
    }
    let _ = stdout.flush();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardBackend {
    Osc52,
    Program(&'static str),
    None,
}

fn is_ssh_session(
    ssh_tty: Option<&str>,
    ssh_connection: Option<&str>,
    ssh_client: Option<&str>,
) -> bool {
    ssh_tty.is_some_and(|v| !v.is_empty())
        || ssh_connection.is_some_and(|v| !v.is_empty())
        || ssh_client.is_some_and(|v| !v.is_empty())
}

fn try_copy_clipboard(text: &str) -> ClipboardBackend {
    if text.is_empty() {
        return ClipboardBackend::None;
    }

    if is_ssh_session(
        env::var("SSH_TTY").ok().as_deref(),
        env::var("SSH_CONNECTION").ok().as_deref(),
        env::var("SSH_CLIENT").ok().as_deref(),
    ) {
        copy_via_osc52(text);
        return ClipboardBackend::Osc52;
    }

    if copy_via_program(&["wl-copy"], text) {
        return ClipboardBackend::Program("wl-copy");
    }
    if copy_via_program(&["xclip", "-selection", "clipboard"], text) {
        return ClipboardBackend::Program("xclip");
    }
    if copy_via_program(&["xsel", "--clipboard", "--input"], text) {
        return ClipboardBackend::Program("xsel");
    }
    if copy_via_program(&["pbcopy"], text) {
        return ClipboardBackend::Program("pbcopy");
    }
    copy_via_osc52(text);
    ClipboardBackend::Osc52
}

fn clipboard_backend_label(backend: ClipboardBackend) -> &'static str {
    match backend {
        ClipboardBackend::Osc52 => "osc52",
        ClipboardBackend::Program(name) => name,
        ClipboardBackend::None => "none",
    }
}

#[derive(Debug, Clone, Copy)]
struct PickerLayout {
    panel_x: usize,
    panel_y: usize,
    panel_w: usize,
    panel_h: usize,
    list_x: usize,
    list_y: usize,
    list_w: usize,
    list_h: usize,
}

fn compute_picker_layout(size: TerminalSize) -> PickerLayout {
    let panel_w = if size.width > 6 {
        (size.width - 6).min(92)
    } else {
        size.width
    };
    let panel_h = if size.height > 4 {
        (size.height - 2).min(24)
    } else {
        size.height
    };
    let panel_x = if size.width > panel_w {
        (size.width - panel_w) / 2
    } else {
        0
    };
    let panel_y = if size.height > panel_h {
        (size.height - panel_h) / 3
    } else {
        0
    };

    let list_x = panel_x + 2;
    let list_y = panel_y + 3;
    let list_w = if panel_w > 4 { panel_w - 4 } else { 0 };
    let list_h = if panel_h > 6 { panel_h - 6 } else { 1 };

    PickerLayout {
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        list_x,
        list_y,
        list_w,
        list_h,
    }
}

fn role_fg(role: Role) -> Color {
    match role {
        Role::User => COLOR_TEXT,
        Role::Assistant => COLOR_TEXT,
        Role::Reasoning => COLOR_TEXT,
        Role::ToolCall => COLOR_TEXT,
        Role::ToolOutput => COLOR_TEXT,
        Role::System => COLOR_DIM,
    }
}

fn role_gutter_fg(role: Role) -> Color {
    match role {
        Role::User => COLOR_GUTTER_USER,
        Role::Assistant => COLOR_GUTTER_AGENT_OUTPUT,
        Role::Reasoning => COLOR_GUTTER_AGENT_THINKING,
        Role::ToolCall => COLOR_GUTTER_TOOL_CALL,
        Role::ToolOutput => COLOR_GUTTER_TOOL_OUTPUT,
        Role::System => COLOR_GUTTER_SYSTEM,
    }
}

fn role_row_bg(role: Role) -> Color {
    match role {
        Role::User => COLOR_ROW_USER,
        Role::Assistant => COLOR_ROW_AGENT_OUTPUT,
        Role::Reasoning => COLOR_ROW_AGENT_THINKING,
        Role::ToolCall => COLOR_ROW_TOOL_CALL,
        Role::ToolOutput => COLOR_ROW_TOOL_OUTPUT,
        Role::System => COLOR_ROW_SYSTEM,
    }
}

fn role_gutter_symbol(role: Role) -> &'static str {
    match role {
        Role::Assistant => " ",
        _ => "┃",
    }
}

fn draw_str(buf: &mut Buffer, x: usize, y: usize, text: &str, style: Style, max_width: usize) {
    if text.is_empty() || max_width == 0 {
        return;
    }
    if let (Ok(x), Ok(y), Ok(w)) = (
        u16::try_from(x),
        u16::try_from(y),
        usize::try_from(max_width),
    ) {
        buf.set_stringn(x, y, text, w, style);
    }
}

fn fill_rect(buf: &mut Buffer, x: usize, y: usize, w: usize, h: usize, style: Style) {
    if w == 0 || h == 0 {
        return;
    }
    if let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
        u16::try_from(x),
        u16::try_from(y),
        u16::try_from(w),
        u16::try_from(h),
    ) {
        buf.set_style(
            Rect {
                x,
                y,
                width: w,
                height: h,
            },
            style,
        );
    }
}

fn draw_rendered_line(
    buf: &mut Buffer,
    x: usize,
    y: usize,
    max_width: usize,
    line: &RenderedLine,
    base_style: Style,
    selection: Option<(usize, usize)>,
) {
    if max_width == 0 || line.cells == 0 {
        return;
    }

    let mut draw_x = x;
    let mut col = 0usize;

    let mut render_segment = |text: &str, seg_style: Style, draw_x: &mut usize, col: &mut usize| {
        if *draw_x >= x + max_width || text.is_empty() {
            return;
        }

        let seg_cells = visual_width(text);
        if seg_cells == 0 {
            return;
        }

        let style = base_style.patch(seg_style);
        let seg_start = *col;
        let seg_end = seg_start + seg_cells;

        let mut draw_piece = |piece: &str, piece_style: Style, draw_x: &mut usize| {
            if piece.is_empty() || *draw_x >= x + max_width {
                return;
            }
            let rem = max_width.saturating_sub(*draw_x - x);
            draw_str(buf, *draw_x, y, piece, piece_style, rem);
            *draw_x += visual_width(piece);
        };

        if let Some((sel_start, sel_end)) = selection {
            if sel_end <= seg_start || sel_start >= seg_end {
                draw_piece(text, style, draw_x);
            } else {
                let local_start = sel_start.saturating_sub(seg_start).min(seg_cells);
                let local_end = sel_end.saturating_sub(seg_start).min(seg_cells);

                let before = slice_by_cells(text, 0, local_start);
                let selected = slice_by_cells(text, local_start, local_end);
                let after = slice_by_cells(text, local_end, seg_cells);

                draw_piece(&before, style, draw_x);
                draw_piece(&selected, style.fg(COLOR_TEXT).bg(COLOR_STEP8), draw_x);
                draw_piece(&after, style, draw_x);
            }
        } else {
            draw_piece(text, style, draw_x);
        }

        *col = seg_end;
    };

    if line.styled_segments.is_empty() {
        render_segment(&line.text, Style::default(), &mut draw_x, &mut col);
        return;
    }

    for seg in &line.styled_segments {
        if draw_x >= x + max_width {
            break;
        }
        render_segment(&seg.text, seg.style, &mut draw_x, &mut col);
    }
}

fn draw_help_overlay(buf: &mut Buffer, size: TerminalSize) {
    if !(size.height > 10 && size.width > 44) {
        return;
    }

    let box_w = (size.width - 8).min(74);
    let box_h = 10usize;
    let start_x = (size.width - box_w) / 2;
    let start_y = (size.height - box_h) / 2;

    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_OVERLAY),
    );
    fill_rect(
        buf,
        start_x,
        start_y,
        box_w,
        box_h,
        Style::default().bg(COLOR_STEP2),
    );

    let left = start_x;
    let right = start_x + box_w - 1;
    let top = start_y;
    let bottom = start_y + box_h - 1;

    draw_str(buf, left, top, "┏", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, right, top, "┓", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, left, bottom, "┗", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, right, bottom, "┛", Style::default().fg(COLOR_STEP7), 1);

    for x in (left + 1)..right {
        draw_str(buf, x, top, "─", Style::default().fg(COLOR_STEP7), 1);
        draw_str(buf, x, bottom, "─", Style::default().fg(COLOR_STEP7), 1);
    }
    for y in (top + 1)..bottom {
        draw_str(buf, left, y, "┃", Style::default().fg(COLOR_STEP7), 1);
        draw_str(buf, right, y, "┃", Style::default().fg(COLOR_STEP7), 1);
    }

    draw_str(
        buf,
        start_x + 3,
        start_y + 1,
        "Help",
        Style::default()
            .fg(COLOR_PRIMARY)
            .add_modifier(Modifier::BOLD),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + box_w.saturating_sub(8),
        start_y + 1,
        "esc",
        Style::default().fg(COLOR_DIM),
        3,
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 3,
        "Enter send/steer  Shift+Enter newline",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 4,
        "Ctrl+Y copy selection or last answer",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 5,
        "g/G or Home/End jump transcript",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 6,
        "Wheel scroll, drag to select, release to copy",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 7,
        "? toggle this help",
        Style::default().fg(COLOR_DIM),
        box_w.saturating_sub(6),
    );
}

fn render_main_view(
    frame: &mut ratatui::Frame<'_>,
    app: &mut AppState,
    rendered_lines: &[RenderedLine],
) {
    let area = frame.area();
    let size = TerminalSize {
        width: area.width as usize,
        height: area.height as usize,
    };

    if size.width == 0 || size.height == 0 {
        return;
    }

    let input_layout = compute_input_layout(app, size);
    let msg_top = MSG_TOP;
    let msg_bottom = input_layout.msg_bottom;
    let msg_height = if msg_bottom >= msg_top {
        msg_bottom - msg_top + 1
    } else {
        0
    };
    let msg_width = transcript_content_width(size);

    let total_lines = rendered_lines.len();
    let max_scroll = total_lines.saturating_sub(msg_height);
    if app.scroll_top > max_scroll {
        app.scroll_top = max_scroll;
    }
    if app.auto_follow_bottom && max_scroll > 0 {
        app.scroll_top = max_scroll;
    }

    let buf = frame.buffer_mut();
    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_STEP1),
    );
    if msg_height > 0 {
        fill_rect(
            buf,
            0,
            msg_top - 1,
            size.width,
            msg_height,
            Style::default().bg(COLOR_STEP2),
        );
    }

    for i in 0..msg_height {
        let line_idx = app.scroll_top + i;
        let row_1b = msg_top + i;
        let y = row_1b - 1;

        let line_opt = rendered_lines.get(line_idx);
        if let Some(line) = line_opt {
            if !line.separator {
                let gutter_symbol = role_gutter_symbol(line.role);
                draw_str(
                    buf,
                    0,
                    y,
                    gutter_symbol,
                    Style::default()
                        .fg(role_gutter_fg(line.role))
                        .add_modifier(Modifier::BOLD),
                    1,
                );
            }
        }

        if msg_width == 0 {
            continue;
        }

        let Some(line) = line_opt else {
            continue;
        };
        fill_rect(
            buf,
            MSG_CONTENT_X,
            y,
            msg_width,
            1,
            Style::default().bg(role_row_bg(line.role)),
        );
        if line.separator {
            let sep = "─".repeat(msg_width);
            draw_str(
                buf,
                MSG_CONTENT_X,
                y,
                &sep,
                Style::default().fg(COLOR_STEP6),
                msg_width,
            );
            continue;
        }

        let mut base_style = Style::default().fg(role_fg(line.role));
        if matches!(line.role, Role::Reasoning) {
            base_style = base_style.add_modifier(Modifier::DIM);
        }

        let selection_range = app
            .selection
            .and_then(|sel| compute_selection_range(sel, row_1b, line.cells))
            .map(|(start, end)| (start.min(line.cells), end.min(line.cells)));

        draw_rendered_line(
            buf,
            MSG_CONTENT_X,
            y,
            msg_width,
            line,
            base_style,
            selection_range,
        );
    }

    if input_layout.input_top > 1 {
        let sep_y = input_layout.input_top - 2;
        if size.width > 0 {
            let corner = if msg_height > 0 { "┗" } else { "━" };
            draw_str(
                buf,
                0,
                sep_y,
                corner,
                Style::default().fg(COLOR_GUTTER_USER),
                1,
            );
            if size.width > 2 {
                let sep = "━".repeat(size.width - 2);
                draw_str(
                    buf,
                    1,
                    sep_y,
                    &sep,
                    Style::default().fg(COLOR_GUTTER_USER),
                    size.width - 2,
                );
            }
        }
    }

    fill_rect(
        buf,
        0,
        input_layout.input_top.saturating_sub(1),
        size.width,
        input_layout.input_height,
        Style::default().bg(COLOR_STEP3),
    );
    for i in 0..input_layout.input_height {
        let y = input_layout.input_top.saturating_sub(1) + i;
        draw_str(
            buf,
            0,
            y,
            ">",
            Style::default()
                .fg(COLOR_GUTTER_USER)
                .add_modifier(Modifier::BOLD),
            1,
        );
        if let Some(line) = input_layout.visible_lines.get(i) {
            draw_str(
                buf,
                MSG_CONTENT_X,
                y,
                line,
                Style::default().fg(COLOR_TEXT),
                input_layout.text_width,
            );
        }
    }

    if app.show_help {
        draw_help_overlay(buf, size);
    }

    let cursor_x = input_layout.cursor_x.min(size.width.saturating_sub(2));
    let cursor_y = input_layout.cursor_y.min(size.height.saturating_sub(1));
    frame.set_cursor_position((cursor_x as u16, cursor_y as u16));
}

fn draw_picker(
    frame: &mut ratatui::Frame<'_>,
    threads: &[ThreadSummary],
    selected: usize,
    top: usize,
) {
    let area = frame.area();
    let size = TerminalSize {
        width: area.width as usize,
        height: area.height as usize,
    };

    if size.width == 0 || size.height == 0 {
        return;
    }

    let layout = compute_picker_layout(size);
    let buf = frame.buffer_mut();

    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_STEP1),
    );
    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_OVERLAY),
    );
    fill_rect(
        buf,
        layout.panel_x,
        layout.panel_y,
        layout.panel_w,
        layout.panel_h,
        Style::default().bg(COLOR_STEP2),
    );

    if layout.panel_w < 8 || layout.panel_h < 7 || layout.list_w == 0 {
        draw_str(
            buf,
            1,
            0,
            "carlos resume",
            Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
            size.width.saturating_sub(1),
        );
        return;
    }

    for y in layout.panel_y..(layout.panel_y + layout.panel_h) {
        draw_str(
            buf,
            layout.panel_x,
            y,
            "┃",
            Style::default().fg(COLOR_STEP7),
            1,
        );
        if layout.panel_w > 1 {
            draw_str(
                buf,
                layout.panel_x + layout.panel_w - 1,
                y,
                "┃",
                Style::default().fg(COLOR_STEP7),
                1,
            );
        }
    }

    draw_str(
        buf,
        layout.list_x,
        layout.panel_y + 1,
        "Sessions",
        Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
        layout.list_w,
    );
    if layout.panel_w > 5 {
        draw_str(
            buf,
            layout.panel_x + layout.panel_w - 5,
            layout.panel_y + 1,
            "esc",
            Style::default().fg(COLOR_DIM),
            3,
        );
    }
    draw_str(
        buf,
        layout.list_x,
        layout.panel_y + 2,
        "Enter open  j/k move  g/G jump",
        Style::default().fg(COLOR_DIM),
        layout.list_w,
    );

    for row in 0..layout.list_h {
        let idx = top + row;
        let y = layout.list_y + row;
        if idx >= threads.len() {
            continue;
        }

        let t = &threads[idx];
        let preview_w = if layout.list_w > 42 {
            layout.list_w - 42
        } else {
            10
        };
        let preview = if visual_width(&t.preview) > preview_w {
            let cut = split_at_cells(&t.preview, preview_w);
            &t.preview[..cut]
        } else {
            &t.preview
        };

        let cwd_tail = if t.cwd.is_empty() { "" } else { &t.cwd };
        let mut line = format!("{}  {}  {}", t.id, preview, cwd_tail);
        if !line.is_empty() {
            let ts = t.updated_at;
            line.push_str(&format!("  {}", ts));
        }
        let view_width = layout.list_w.saturating_sub(2);
        if visual_width(&line) > view_width {
            let cut = split_at_cells(&line, view_width);
            line.truncate(cut);
        }

        let active = idx == selected;
        if active && layout.list_w > 0 {
            fill_rect(
                buf,
                layout.list_x,
                y,
                layout.list_w,
                1,
                Style::default().bg(COLOR_PRIMARY),
            );
        }

        let bullet_style = if active {
            Style::default().fg(COLOR_STEP1).bg(COLOR_PRIMARY)
        } else {
            Style::default().fg(COLOR_DIM)
        };
        let line_style = if active {
            Style::default()
                .fg(COLOR_STEP1)
                .bg(COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(COLOR_TEXT)
        };

        draw_str(
            buf,
            layout.list_x,
            y,
            if active { "●" } else { " " },
            bullet_style,
            1,
        );
        draw_str(buf, layout.list_x + 2, y, &line, line_style, view_width);
    }

    draw_str(
        buf,
        layout.list_x,
        layout.panel_y + layout.panel_h - 2,
        &format!("{} sessions", threads.len()),
        Style::default().fg(COLOR_DIM),
        layout.list_w,
    );
}

fn normalize_selection_x(col0: usize) -> usize {
    if col0 >= MSG_CONTENT_X {
        col0 - MSG_CONTENT_X + 1
    } else {
        1
    }
}

fn is_ctrl_char(code: KeyCode, modifiers: KeyModifiers, ch: char) -> bool {
    matches!(code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&ch))
        && modifiers.contains(KeyModifiers::CONTROL)
}

fn handle_notification_line(app: &mut AppState, line: &str) {
    let Ok(parsed) = serde_json::from_str::<Value>(line) else {
        return;
    };
    let Some(method) = parsed.get("method").and_then(Value::as_str) else {
        return;
    };
    let Some(params) = parsed.get("params").and_then(Value::as_object) else {
        return;
    };

    match method {
        "turn/started" => {
            if let Some(id) = params
                .get("turn")
                .and_then(Value::as_object)
                .and_then(|t| t.get("id"))
                .and_then(Value::as_str)
            {
                app.active_turn_id = Some(id.to_string());
                app.auto_follow_bottom = true;
                app.set_status("turn started");
            }
        }
        "turn/completed" => {
            app.active_turn_id = None;
            app.set_status("turn completed");
        }
        "turn/diff/updated" => {
            if let (Some(turn_id), Some(diff)) = (
                params.get("turnId").and_then(Value::as_str),
                params.get("diff").and_then(Value::as_str),
            ) {
                app.upsert_turn_diff(turn_id, diff);
            }
        }
        "codex/event/turn_diff" => {
            let turn_id = params
                .get("id")
                .and_then(Value::as_str)
                .or_else(|| params.get("turnId").and_then(Value::as_str));
            let diff = params
                .get("msg")
                .and_then(|m| m.get("unified_diff"))
                .and_then(Value::as_str)
                .or_else(|| params.get("diff").and_then(Value::as_str));
            if let (Some(turn_id), Some(diff)) = (turn_id, diff) {
                app.upsert_turn_diff(turn_id, diff);
            }
        }
        "item/started" => {
            let Some(item) = params.get("item").and_then(Value::as_object) else {
                return;
            };
            let Some(t) = item.get("type").and_then(Value::as_str) else {
                return;
            };

            match t {
                "userMessage" => {
                    if let Some(content) = item.get("content").and_then(Value::as_array) {
                        for part in content {
                            if part.get("type").and_then(Value::as_str) != Some("text") {
                                continue;
                            }
                            if let Some(text) = part.get("text").and_then(Value::as_str) {
                                app.append_message(Role::User, text.to_string());
                            }
                        }
                    }
                }
                "agentMessage" => {
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        let idx = app.append_message(Role::Assistant, String::new());
                        app.put_agent_item_mapping(id, idx);
                    }
                }
                "reasoning" => {
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        let idx = app.append_message(Role::Reasoning, String::new());
                        app.put_agent_item_mapping(id, idx);
                    }
                }
                "commandExecution" => {
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        let idx = app.append_message(Role::ToolCall, String::new());
                        app.put_agent_item_mapping(id, idx);
                    }
                }
                t if is_tool_call_type(t) => {
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        let idx = app.append_message(Role::ToolCall, String::new());
                        app.put_agent_item_mapping(id, idx);
                    }
                }
                t if is_tool_output_type(t) => {
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        let idx = app.append_message(Role::ToolOutput, String::new());
                        app.put_agent_item_mapping(id, idx);
                    }
                }
                _ => {}
            }
        }
        "item/completed" => {
            let Some(item) = params.get("item").and_then(Value::as_object) else {
                return;
            };
            let Some(kind) = item.get("type").and_then(Value::as_str) else {
                return;
            };
            let Some(mut role) = role_for_tool_type(kind) else {
                return;
            };
            if kind == "commandExecution" {
                role = Role::ToolOutput;
            }

            let item_value = Value::Object(item.clone());
            let diffs = extract_diff_blocks(&item_value);
            if diffs.is_empty() {
                if let Some(formatted) = format_tool_item(&item_value, role) {
                    let item_id = item.get("id").and_then(Value::as_str);
                    if let Some(id) = item_id {
                        if let Some(idx) = app.agent_item_to_index.get(id).copied() {
                            if let Some(msg) = app.messages.get_mut(idx) {
                                msg.role = role;
                                msg.text = formatted;
                                msg.kind = MessageKind::Plain;
                                msg.file_path = None;
                            }
                            app.auto_follow_bottom = true;
                            return;
                        }
                    }
                    app.append_message(role, formatted);
                    app.auto_follow_bottom = true;
                }
                return;
            }

            let item_id = item.get("id").and_then(Value::as_str);
            if let Some(id) = item_id {
                if let Some(idx) = app.agent_item_to_index.get(id).copied() {
                    if let Some(first) = diffs.first() {
                        if let Some(msg) = app.messages.get_mut(idx) {
                            msg.role = role;
                            msg.text = first.diff.clone();
                            msg.kind = MessageKind::Diff;
                            msg.file_path = first.file_path.clone();
                        }
                        for block in diffs.iter().skip(1) {
                            app.append_diff_message(
                                role,
                                block.file_path.clone(),
                                block.diff.clone(),
                            );
                        }
                        app.auto_follow_bottom = true;
                        return;
                    }
                }
            }

            for block in diffs {
                app.append_diff_message(role, block.file_path, block.diff);
            }
            app.auto_follow_bottom = true;
        }
        "item/agentMessage/delta" => {
            if let (Some(item_id), Some(delta)) = (
                params.get("itemId").and_then(Value::as_str),
                params.get("delta").and_then(Value::as_str),
            ) {
                app.upsert_agent_delta(item_id, delta);
                app.auto_follow_bottom = true;
            }
        }
        "item/reasoning/summaryTextDelta" => {
            if let (Some(item_id), Some(delta)) = (
                params.get("itemId").and_then(Value::as_str),
                params.get("delta").and_then(Value::as_str),
            ) {
                app.upsert_agent_delta(item_id, delta);
            }
        }
        "item/toolCall/delta"
        | "item/tool_call/delta"
        | "item/toolInvocation/delta"
        | "item/functionCall/delta"
        | "item/mcpToolCall/delta"
        | "item/toolResult/delta"
        | "item/toolOutput/delta"
        | "item/tool_result/delta"
        | "item/functionCallOutput/delta"
        | "item/mcpToolResult/delta" => {
            if let (Some(item_id), Some(delta)) = (
                params.get("itemId").and_then(Value::as_str),
                params.get("delta").and_then(Value::as_str),
            ) {
                app.upsert_agent_delta(item_id, delta);
                app.auto_follow_bottom = true;
            }
        }
        "error" => {
            let msg = params
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("server error");
            app.set_status(msg);
        }
        _ => {}
    }
}

fn with_terminal<T>(
    f: impl FnOnce(&mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<T>,
) -> Result<T> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .context("failed to enter alt screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to create terminal")?;

    let result = f(&mut terminal);

    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();

    result
}

fn pick_thread(threads: &[ThreadSummary]) -> Result<Option<String>> {
    if threads.is_empty() {
        return Ok(None);
    }

    with_terminal(|terminal| {
        let mut selected = 0usize;
        let mut top = 0usize;
        let mut last_size = TerminalSize {
            width: 0,
            height: 0,
        };

        loop {
            terminal.draw(|frame| {
                let area = frame.area();
                let size = TerminalSize {
                    width: area.width as usize,
                    height: area.height as usize,
                };

                if size.width != last_size.width || size.height != last_size.height {
                    last_size = size;
                }

                let layout = compute_picker_layout(size);
                let list_height = layout.list_h.max(1);
                if selected < top {
                    top = selected;
                }
                if selected >= top + list_height {
                    top = selected + 1 - list_height;
                }

                draw_picker(frame, threads, selected, top);
            })?;

            if !event::poll(Duration::from_millis(15))? {
                continue;
            }

            let ev = event::read()?;
            match ev {
                Event::Key(k) if k.kind == KeyEventKind::Press => match (k.code, k.modifiers) {
                    (code, mods) if is_ctrl_char(code, mods, 'c') => return Ok(None),
                    (KeyCode::Esc, _) => return Ok(None),
                    (KeyCode::Up, _) => {
                        selected = selected.saturating_sub(1);
                    }
                    (KeyCode::Down, _) => {
                        if selected + 1 < threads.len() {
                            selected += 1;
                        }
                    }
                    (KeyCode::PageUp, _) => {
                        selected = selected.saturating_sub(10);
                    }
                    (KeyCode::PageDown, _) => {
                        selected = (selected + 10).min(threads.len().saturating_sub(1));
                    }
                    (KeyCode::Home, _) | (KeyCode::Char('g'), _) => selected = 0,
                    (KeyCode::End, _) | (KeyCode::Char('G'), _) => {
                        selected = threads.len().saturating_sub(1)
                    }
                    (KeyCode::Char('j'), _) => {
                        if selected + 1 < threads.len() {
                            selected += 1;
                        }
                    }
                    (KeyCode::Char('k'), _) => {
                        selected = selected.saturating_sub(1);
                    }
                    (KeyCode::Enter, _) => return Ok(Some(threads[selected].id.clone())),
                    _ => {}
                },
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::ScrollUp => {
                        selected = selected.saturating_sub(1);
                    }
                    MouseEventKind::ScrollDown => {
                        if selected + 1 < threads.len() {
                            selected += 1;
                        }
                    }
                    MouseEventKind::Down(MouseButton::Left) => {
                        let size = terminal.size()?;
                        let layout = compute_picker_layout(TerminalSize {
                            width: size.width as usize,
                            height: size.height as usize,
                        });
                        let row0 = m.row as usize;
                        if row0 >= layout.list_y && row0 < layout.list_y + layout.list_h {
                            let idx = top + (row0 - layout.list_y);
                            if idx < threads.len() {
                                selected = idx;
                                return Ok(Some(threads[selected].id.clone()));
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    })
}

fn run_conversation_tui(client: &AppServerClient, app: &mut AppState) -> Result<()> {
    with_terminal(|terminal| {
        let mut inbox = Vec::new();

        loop {
            inbox.clear();
            client.drain_events(&mut inbox);
            for line in &inbox {
                handle_notification_line(app, line);
            }

            let size = terminal.size()?;
            let size = TerminalSize {
                width: size.width as usize,
                height: size.height as usize,
            };
            let rendered = build_rendered_lines(&app.messages, transcript_content_width(size));

            terminal.draw(|frame| {
                render_main_view(frame, app, &rendered);
            })?;

            let mut had_input = false;
            let tick_start = Instant::now();
            while tick_start.elapsed() < Duration::from_millis(16)
                && event::poll(Duration::from_millis(0))?
            {
                had_input = true;
                match event::read()? {
                    Event::Key(k) if k.kind == KeyEventKind::Press => {
                        if app.show_help {
                            match (k.code, k.modifiers) {
                                (code, mods) if is_ctrl_char(code, mods, 'c') => return Ok(()),
                                (KeyCode::Esc, _) => {
                                    app.show_help = false;
                                }
                                (KeyCode::Char('?'), _) => {
                                    app.show_help = false;
                                }
                                _ => {}
                            }
                            continue;
                        }

                        match (k.code, k.modifiers) {
                            (code, mods) if is_ctrl_char(code, mods, 'c') => return Ok(()),
                            (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
                                if let Some(sel) = app.selection {
                                    let msg_bottom = compute_input_layout(app, size).msg_bottom;
                                    let copied =
                                        selected_text(sel, &rendered, msg_bottom, app.scroll_top);
                                    if !copied.is_empty() {
                                        let _ = try_copy_clipboard(&copied);
                                    }
                                } else if let Some(text) = last_assistant_message(&app.messages) {
                                    let backend = try_copy_clipboard(text);
                                    app.set_status(format!(
                                        "copied last assistant message ({})",
                                        clipboard_backend_label(backend)
                                    ));
                                }
                            }
                            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                                app.selection = None;
                                app.set_status("selection cleared");
                            }
                            (KeyCode::Esc, _) => {
                                app.selection = None;
                                app.set_status("selection cleared");
                            }
                            (KeyCode::Home, _) if app.input_is_empty() => {
                                app.auto_follow_bottom = false;
                                app.scroll_top = 0;
                            }
                            (KeyCode::End, _) if app.input_is_empty() => {
                                app.auto_follow_bottom = true;
                            }
                            (KeyCode::Up, _) if app.input_is_empty() => {
                                app.auto_follow_bottom = false;
                                app.scroll_top = app.scroll_top.saturating_sub(1);
                            }
                            (KeyCode::Down, _) if app.input_is_empty() => {
                                app.auto_follow_bottom = false;
                                app.scroll_top = app.scroll_top.saturating_add(1);
                            }
                            (KeyCode::PageUp, _) => {
                                app.auto_follow_bottom = false;
                                app.scroll_top = app.scroll_top.saturating_sub(10);
                            }
                            (KeyCode::PageDown, _) => {
                                app.auto_follow_bottom = false;
                                app.scroll_top = app.scroll_top.saturating_add(10);
                            }
                            (KeyCode::Enter, mods) if mods.contains(KeyModifiers::SHIFT) => {
                                let _ = app.input.input(textarea_input_from_key(k));
                            }
                            (KeyCode::Enter, _) => {
                                if app.input_is_empty() {
                                    continue;
                                }

                                let text = app.input_text();
                                app.clear_input();
                                app.selection = None;

                                if let Some(turn_id) = app.active_turn_id.clone() {
                                    let params = params_turn_steer(&app.thread_id, &turn_id, &text);
                                    match client.call("turn/steer", params, Duration::from_secs(10))
                                    {
                                        Ok(_) => app.set_status("sent steer"),
                                        Err(e) => app.set_status(format!("{e}")),
                                    }
                                } else {
                                    let params = params_turn_start(&app.thread_id, &text);
                                    match client.call("turn/start", params, Duration::from_secs(10))
                                    {
                                        Ok(_) => app.set_status("sent turn"),
                                        Err(e) => app.set_status(format!("{e}")),
                                    }
                                }
                            }
                            (KeyCode::Char('?'), _) => {
                                if app.input_is_empty() {
                                    app.show_help = true;
                                } else {
                                    let _ = app.input.input(textarea_input_from_key(k));
                                }
                            }
                            (KeyCode::Char('g'), _) if app.input_is_empty() => {
                                app.auto_follow_bottom = false;
                                app.scroll_top = 0;
                            }
                            (KeyCode::Char('G'), _) if app.input_is_empty() => {
                                app.auto_follow_bottom = true;
                            }
                            _ => {
                                let _ = app.input.input(textarea_input_from_key(k));
                            }
                        }
                    }
                    Event::Mouse(m) => {
                        if app.show_help {
                            continue;
                        }

                        let msg_top = MSG_TOP;
                        let msg_bottom = compute_input_layout(app, size).msg_bottom;
                        if msg_bottom < msg_top {
                            continue;
                        }

                        let row1 = m.row as usize + 1;
                        let in_messages = row1 >= msg_top && row1 <= msg_bottom;
                        let norm_x = normalize_selection_x(m.column as usize);
                        let clamped_y = row1.clamp(msg_top, msg_bottom);

                        match m.kind {
                            MouseEventKind::ScrollUp => {
                                app.auto_follow_bottom = false;
                                app.scroll_top = app.scroll_top.saturating_sub(3);
                            }
                            MouseEventKind::ScrollDown => {
                                app.auto_follow_bottom = false;
                                app.scroll_top = app.scroll_top.saturating_add(3);
                            }
                            MouseEventKind::Down(MouseButton::Left) => {
                                if in_messages {
                                    app.selection = Some(Selection {
                                        anchor_x: norm_x,
                                        anchor_y: row1,
                                        focus_x: norm_x,
                                        focus_y: row1,
                                        dragging: true,
                                    });
                                }
                            }
                            MouseEventKind::Drag(MouseButton::Left) => {
                                if let Some(sel) = app.selection.as_mut() {
                                    if sel.dragging {
                                        sel.focus_x = norm_x;
                                        sel.focus_y = clamped_y;
                                    }
                                }
                            }
                            MouseEventKind::Up(MouseButton::Left) => {
                                if let Some(sel) = app.selection.as_mut() {
                                    if sel.dragging {
                                        sel.focus_x = norm_x;
                                        sel.focus_y = clamped_y;
                                        sel.dragging = false;

                                        let copied = selected_text(
                                            *sel,
                                            &rendered,
                                            msg_bottom,
                                            app.scroll_top,
                                        );
                                        if !copied.is_empty() {
                                            let _ = try_copy_clipboard(&copied);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Event::Resize(_, _) => {}
                    _ => {}
                }
            }

            if !had_input {
                thread::sleep(Duration::from_millis(16));
            }
        }
    })
}

fn usage() {
    eprintln!("Usage:\n  carlos\n  carlos resume [SESSION_ID]");
}

fn run() -> Result<()> {
    let mut args = env::args();
    let _bin = args.next();

    let cmd = args.next();
    let mut mode_resume = false;
    let mut resume_id: Option<String> = None;

    if let Some(cmd) = cmd {
        if cmd == "resume" {
            mode_resume = true;
            resume_id = args.next();
        } else {
            usage();
            return Ok(());
        }
    }

    let client = AppServerClient::start()?;
    initialize_client(&client)?;

    let cwd = env::current_dir()?.to_string_lossy().to_string();

    let (chosen_thread_id, start_resp) = if mode_resume {
        if let Some(rid) = resume_id {
            let resp = client.call(
                "thread/resume",
                params_thread_resume(&rid),
                Duration::from_secs(20),
            )?;
            let thread_id = parse_thread_id_from_start_or_resume(&resp)?;
            (thread_id, resp)
        } else {
            let list_resp = client.call(
                "thread/list",
                params_thread_list(&cwd),
                Duration::from_secs(15),
            )?;
            let list = parse_thread_list(&list_resp)?;
            let picked = pick_thread(&list)?;
            let Some(session_id) = picked else {
                return Ok(());
            };

            let resp = client.call(
                "thread/resume",
                params_thread_resume(&session_id),
                Duration::from_secs(20),
            )?;
            let thread_id = parse_thread_id_from_start_or_resume(&resp)?;
            (thread_id, resp)
        }
    } else {
        let resp = client.call(
            "thread/start",
            params_thread_start(&cwd),
            Duration::from_secs(20),
        )?;
        let thread_id = parse_thread_id_from_start_or_resume(&resp)?;
        (thread_id, resp)
    };

    let mut app = AppState::new(chosen_thread_id);
    load_history_from_start_or_resume(&mut app, &start_resp)?;
    app.set_status("ready");

    run_conversation_tui(&client, &mut app)
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_selection_range_normalizes_reversed_coordinates() {
        let sel = Selection {
            anchor_x: 10,
            anchor_y: 5,
            focus_x: 3,
            focus_y: 5,
            dragging: false,
        };
        let range = compute_selection_range(sel, 5, 20).unwrap();
        assert_eq!(range, (2, 10));
    }

    #[test]
    fn selected_text_keeps_left_padding_in_selection() {
        let lines = vec![RenderedLine {
            text: "  hello".to_string(),
            styled_segments: Vec::new(),
            role: Role::Assistant,
            separator: false,
            cells: 7,
            soft_wrap_to_next: false,
        }];

        let sel = Selection {
            anchor_x: 1,
            anchor_y: MSG_TOP,
            focus_x: 4,
            focus_y: MSG_TOP,
            dragging: false,
        };

        let out = selected_text(sel, &lines, 19, 0);
        assert_eq!(out, "  he");
    }

    #[test]
    fn selected_text_joins_soft_wrapped_rows_without_newline() {
        let lines = vec![
            RenderedLine {
                text: "abcde".to_string(),
                styled_segments: Vec::new(),
                role: Role::Assistant,
                separator: false,
                cells: 5,
                soft_wrap_to_next: true,
            },
            RenderedLine {
                text: "fghij".to_string(),
                styled_segments: Vec::new(),
                role: Role::Assistant,
                separator: false,
                cells: 5,
                soft_wrap_to_next: false,
            },
        ];

        let sel = Selection {
            anchor_x: 1,
            anchor_y: MSG_TOP,
            focus_x: 5,
            focus_y: MSG_TOP + 1,
            dragging: false,
        };

        let out = selected_text(sel, &lines, 19, 0);
        assert_eq!(out, "abcdefghij");
    }

    #[test]
    fn selected_text_keeps_newline_on_hard_break_rows() {
        let lines = vec![
            RenderedLine {
                text: "abcde".to_string(),
                styled_segments: Vec::new(),
                role: Role::Assistant,
                separator: false,
                cells: 5,
                soft_wrap_to_next: false,
            },
            RenderedLine {
                text: "fghij".to_string(),
                styled_segments: Vec::new(),
                role: Role::Assistant,
                separator: false,
                cells: 5,
                soft_wrap_to_next: false,
            },
        ];

        let sel = Selection {
            anchor_x: 1,
            anchor_y: MSG_TOP,
            focus_x: 5,
            focus_y: MSG_TOP + 1,
            dragging: false,
        };

        let out = selected_text(sel, &lines, 19, 0);
        assert_eq!(out, "abcde\nfghij");
    }

    #[test]
    fn build_rendered_lines_inserts_separator_rows_between_messages() {
        let messages = vec![
            Message {
                role: Role::User,
                text: "first".to_string(),
                kind: MessageKind::Plain,
                file_path: None,
            },
            Message {
                role: Role::Assistant,
                text: "second".to_string(),
                kind: MessageKind::Plain,
                file_path: None,
            },
        ];

        let rendered = build_rendered_lines(&messages, 40);
        assert!(rendered.len() >= 3);
        assert!(rendered[1].separator);
        assert_eq!(rendered[1].role, Role::System);
    }

    #[test]
    fn wrap_natural_by_cells_prefers_word_boundaries() {
        let parts = wrap_natural_by_cells("alpha beta gamma", 10);
        assert_eq!(parts, vec!["alpha beta".to_string(), "gamma".to_string()]);
    }

    #[test]
    fn build_rendered_lines_hides_markdown_fence_delimiters() {
        let messages = vec![Message {
            role: Role::Assistant,
            text: "```zig\nconst x = 1;\n```\n".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        }];

        let rendered = build_rendered_lines(&messages, 60);
        assert!(rendered.iter().all(|l| !l.text.contains("```")));
    }

    #[test]
    fn build_rendered_lines_styles_assistant_code_lines() {
        let messages = vec![Message {
            role: Role::Assistant,
            text: "```rust\nfn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n```".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        }];

        let rendered = build_rendered_lines(&messages, 120);
        let line = rendered
            .iter()
            .find(|l| l.text.contains("fn add"))
            .expect("expected highlighted code line");

        assert!(!line.styled_segments.is_empty());
        assert!(line
            .styled_segments
            .iter()
            .any(|s| s.style != Style::default()));
    }

    #[test]
    fn build_rendered_lines_reasoning_uses_markdown_text_without_markers() {
        let messages = vec![Message {
            role: Role::Reasoning,
            text: "**Committing cleanup with style**".to_string(),
            kind: MessageKind::Plain,
            file_path: None,
        }];

        let rendered = build_rendered_lines(&messages, 120);
        assert_eq!(rendered.len(), 1);
        assert_eq!(rendered[0].text, "Committing cleanup with style");
        assert!(!rendered[0].text.contains("Thinking:"));
        assert!(!rendered[0].text.contains("**"));
    }

    #[test]
    fn extract_diff_blocks_reads_nested_metadata_files() {
        let item = json!({
            "type": "toolOutput",
            "metadata": {
                "files": [
                    {
                        "filePath": "src/main.rs",
                        "diff": "@@ -1,1 +1,1 @@\n-old\n+new\n"
                    }
                ]
            }
        });

        let blocks = extract_diff_blocks(&item);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].file_path.as_deref(), Some("src/main.rs"));
        assert!(blocks[0].diff.contains("@@ -1,1 +1,1 @@"));
    }

    #[test]
    fn build_rendered_lines_diff_styles_added_and_removed_lines() {
        let messages = vec![Message {
            role: Role::ToolOutput,
            text: "@@ -1,1 +1,1 @@\n-old\n+new\n".to_string(),
            kind: MessageKind::Diff,
            file_path: Some("src/main.rs".to_string()),
        }];

        let rendered = build_rendered_lines(&messages, 120);
        let removed = rendered
            .iter()
            .find(|l| l.text == "-old")
            .expect("missing removed line");
        let added = rendered
            .iter()
            .find(|l| l.text == "+new")
            .expect("missing added line");

        assert_eq!(removed.styled_segments[0].style.fg, Some(COLOR_DIFF_REMOVE));
        assert_eq!(added.styled_segments[0].style.fg, Some(COLOR_DIFF_ADD));
    }

    #[test]
    fn handle_notification_turn_diff_updated_upserts_diff_message() {
        let mut app = AppState::new("thread-1".to_string());
        handle_notification_line(
            &mut app,
            "{\"method\":\"turn/diff/updated\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"diff\":\"diff --git a/test.txt b/test.txt\\n@@ -1 +1 @@\\n-old\\n+new\\n\"}}",
        );

        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].role, Role::ToolOutput);
        assert_eq!(app.messages[0].kind, MessageKind::Diff);
        assert!(app.messages[0].text.contains("+new"));

        handle_notification_line(
            &mut app,
            "{\"method\":\"turn/diff/updated\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-1\",\"diff\":\"diff --git a/test.txt b/test.txt\\n@@ -1 +1 @@\\n-old\\n+newer\\n\"}}",
        );

        assert_eq!(app.messages.len(), 1);
        assert!(app.messages[0].text.contains("+newer"));
    }

    #[test]
    fn handle_notification_codex_event_turn_diff_adds_diff_message() {
        let mut app = AppState::new("thread-1".to_string());
        handle_notification_line(
            &mut app,
            "{\"method\":\"codex/event/turn_diff\",\"params\":{\"id\":\"turn-2\",\"msg\":{\"type\":\"turn_diff\",\"unified_diff\":\"diff --git a/a b/a\\n@@ -1 +1 @@\\n-a\\n+b\\n\"}}}",
        );

        assert_eq!(app.messages.len(), 1);
        assert_eq!(app.messages[0].kind, MessageKind::Diff);
        assert!(app.messages[0].text.contains("+b"));
    }

    #[test]
    fn format_tool_item_run_style_from_command_fields() {
        let item = json!({
            "type": "toolCall",
            "input": {
                "command": "cargo test",
                "reasoning": "Running cargo test in repo",
                "description": "Runs Rust test suite using Cargo"
            }
        });

        let rendered = format_tool_item(&item, Role::ToolCall).expect("formatted tool call");
        assert!(rendered.contains("run `cargo test`"));
        assert!(rendered.contains("Thinking: Running cargo test in repo"));
        assert!(rendered.contains("# Runs Rust test suite using Cargo"));
        assert!(rendered.contains("$ cargo test"));
    }

    #[test]
    fn format_tool_item_collects_stdout_stderr_and_exit_code() {
        let item = json!({
            "type": "toolOutput",
            "stdout": "Finished `test` profile [optimized + debuginfo] target(s) in 0.04s",
            "stderr": "",
            "exitCode": 0
        });

        let rendered = format_tool_item(&item, Role::ToolOutput).expect("formatted output");
        assert!(rendered.contains("Finished `test` profile"));
        assert!(rendered.contains("exit code: 0"));
    }

    #[test]
    fn format_tool_item_read_call_shows_offset_bracket() {
        let item = json!({
            "type": "toolCall",
            "tool": "read",
            "input": {
                "filePath": "src/main.rs",
                "offset": 1791
            }
        });

        let rendered = format_tool_item(&item, Role::ToolCall).expect("formatted read call");
        assert_eq!(rendered, "→ Read src/main.rs [offset=1791]");
    }

    #[test]
    fn format_command_execution_call_uses_action_command() {
        let item = json!({
            "type": "commandExecution",
            "id": "call_1",
            "command": "/usr/bin/zsh -lc 'ls -1'",
            "commandActions": [
                { "type": "listFiles", "command": "ls -1", "path": null }
            ],
            "status": "inProgress"
        });

        let rendered = format_tool_item(&item, Role::ToolCall).expect("formatted command call");
        assert_eq!(rendered, "run `ls -1`\n$ ls -1");
    }

    #[test]
    fn format_command_execution_output_uses_aggregated_output() {
        let item = json!({
            "type": "commandExecution",
            "id": "call_1",
            "aggregatedOutput": "a\nb\n",
            "exitCode": 0,
            "durationMs": 51,
            "status": "completed"
        });

        let rendered = format_tool_item(&item, Role::ToolOutput).expect("formatted command output");
        assert!(rendered.contains("a\nb"), "rendered={rendered:?}");
        assert!(rendered.contains("exit code: 0"), "rendered={rendered:?}");
    }

    #[test]
    fn widechar_selection_uses_cell_offsets() {
        let line = "a😀b";
        assert_eq!(visual_width(line), 4);
        let s = slice_by_cells(line, 1, 3);
        assert_eq!(s, "😀");
    }

    #[test]
    fn osc52_wrap_detects_tmux_and_screen() {
        assert_eq!(
            detect_osc52_wrap(Some("/tmp/tmux-1000/default,123,0"), Some("xterm-256color")),
            Osc52Wrap::Tmux
        );
        assert_eq!(
            detect_osc52_wrap(None, Some("screen-256color")),
            Osc52Wrap::Screen
        );
        assert_eq!(
            detect_osc52_wrap(None, Some("xterm-256color")),
            Osc52Wrap::None
        );
    }

    #[test]
    fn osc52_tmux_sequence_uses_passthrough_and_escaped_esc() {
        let encoded = "YQ==";
        let seqs = osc52_sequences_for_env(encoded, Some("1"), Some("xterm-256color"));
        let first = &seqs[0];
        assert!(first.starts_with("\x1bPtmux;"));
        assert!(first.contains("\x1b\x1b]52;c;YQ=="));
        assert!(first.ends_with("\x1b\\"));
    }

    #[test]
    fn osc52_screen_sequence_uses_dcs_wrapper() {
        let encoded = "YQ==";
        let seqs = osc52_sequences_for_env(encoded, None, Some("screen-256color"));
        let first = &seqs[0];
        assert!(first.starts_with("\x1bP\x1b]52;c;YQ=="));
        assert!(first.ends_with("\x1b\\"));
    }

    #[test]
    fn osc52_generates_both_clipboard_targets() {
        let seqs = osc52_sequences_for_env("YQ==", None, Some("xterm-256color"));
        assert!(seqs.iter().any(|s| s.contains("]52;c;YQ==")));
        assert!(seqs.iter().any(|s| s.contains("]52;p;YQ==")));
    }

    #[test]
    fn ssh_detection_works() {
        assert!(is_ssh_session(Some("/dev/pts/3"), None, None));
        assert!(is_ssh_session(None, Some("1.2.3.4 22 5.6.7.8 54321"), None));
        assert!(is_ssh_session(None, None, Some("1.2.3.4 54321 22")));
        assert!(!is_ssh_session(None, None, None));
    }
}
