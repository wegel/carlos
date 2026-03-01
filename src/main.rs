use std::collections::HashMap;
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
use serde_json::{json, Value};
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
const COLOR_ROW_AGENT_OUTPUT: Color = Color::Rgb(30, 30, 46);
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
struct RenderedLine {
    text: String,
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

    input: String,
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
            input: String::new(),
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

    fn append_message(&mut self, role: Role, text: impl Into<String>) -> usize {
        self.messages.push(Message {
            role,
            text: text.into(),
        });
        self.messages.len() - 1
    }

    fn put_agent_item_mapping(&mut self, item_id: &str, idx: usize) {
        self.agent_item_to_index.insert(item_id.to_string(), idx);
    }

    fn upsert_agent_delta(&mut self, item_id: &str, delta: &str) {
        if let Some(idx) = self.agent_item_to_index.get(item_id).copied() {
            if let Some(msg) = self.messages.get_mut(idx) {
                msg.text.push_str(delta);
            }
            return;
        }

        let idx = self.append_message(Role::Assistant, delta);
        self.put_agent_item_mapping(item_id, idx);
    }
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
        Role::Reasoning => "Thinking: ",
        Role::ToolCall => "Tool: ",
        Role::ToolOutput => "Result: ",
        Role::System => "",
    }
}

fn is_fence_delimiter(line: &str) -> bool {
    line.trim_matches([' ', '\t', '\r']).starts_with("```")
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

        let mut rest = logical;
        let continuation = match role {
            Role::Reasoning => "          ",
            Role::ToolCall => "      ",
            Role::ToolOutput => "        ",
            _ => role_prefix(role),
        };

        if rest.is_empty() {
            let t = prefix.to_string();
            out.push(RenderedLine {
                cells: visual_width(&t),
                text: t,
                role,
                separator: false,
                soft_wrap_to_next: false,
            });
            first_physical = false;
            continue;
        }

        loop {
            let lead = if in_code_fence {
                role_prefix(Role::System)
            } else if first_physical {
                prefix
            } else {
                continuation
            };
            let lead_cells = visual_width(lead);
            let avail = width.saturating_sub(lead_cells);
            if avail == 0 {
                break;
            }

            let take = split_at_cells(rest, avail);
            if take == 0 {
                break;
            }

            let mut line = String::with_capacity(lead.len() + take);
            line.push_str(lead);
            line.push_str(&rest[..take]);
            let wrapped = take < rest.len();

            out.push(RenderedLine {
                cells: visual_width(&line),
                text: line,
                role,
                separator: false,
                soft_wrap_to_next: wrapped,
            });

            if !wrapped {
                break;
            }
            rest = &rest[take..];
            first_physical = false;
        }

        first_physical = false;
    }
}

fn build_rendered_lines(messages: &[Message], width: usize) -> Vec<RenderedLine> {
    let mut out = Vec::new();

    for (idx, msg) in messages.iter().enumerate() {
        if idx > 0 {
            out.push(RenderedLine {
                text: String::new(),
                role: Role::System,
                separator: true,
                cells: 0,
                soft_wrap_to_next: false,
            });
        }
        append_wrapped_message_lines(&mut out, msg.role, &msg.text, width);
    }

    out
}

fn transcript_content_width(size: TerminalSize) -> usize {
    if size.width > MSG_CONTENT_X {
        size.width - MSG_CONTENT_X
    } else {
        0
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
    size: TerminalSize,
    scroll_top: usize,
) -> String {
    let msg_top = MSG_TOP;
    let msg_bottom = size.height.saturating_sub(1);

    let mut ax = selection.anchor_x;
    let mut ay = selection.anchor_y;
    let mut fx = selection.focus_x;
    let mut fy = selection.focus_y;
    if fy < ay || (fy == ay && fx < ax) {
        std::mem::swap(&mut ax, &mut fx);
        std::mem::swap(&mut ay, &mut fy);
    }

    if fy < msg_top || ay > msg_bottom {
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
        "Enter send/steer",
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

    let msg_top = MSG_TOP;
    let msg_bottom = size.height.saturating_sub(1);
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
                draw_str(
                    buf,
                    0,
                    y,
                    "┃",
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

        if let Some(sel) = app.selection {
            if let Some((start, end)) = compute_selection_range(sel, row_1b, line.cells) {
                let start = start.min(line.cells);
                let end = end.min(line.cells);

                let before = slice_by_cells(&line.text, 0, start);
                let selected = slice_by_cells(&line.text, start, end);
                let after = slice_by_cells(&line.text, end, line.cells);

                let mut x = MSG_CONTENT_X;
                if !before.is_empty() {
                    let w = visual_width(&before);
                    draw_str(
                        buf,
                        x,
                        y,
                        &before,
                        base_style,
                        msg_width.saturating_sub(x - MSG_CONTENT_X),
                    );
                    x += w;
                }
                if !selected.is_empty() {
                    let w = visual_width(&selected);
                    draw_str(
                        buf,
                        x,
                        y,
                        &selected,
                        base_style.fg(COLOR_TEXT).bg(COLOR_STEP8),
                        msg_width.saturating_sub(x - MSG_CONTENT_X),
                    );
                    x += w;
                }
                if !after.is_empty() {
                    draw_str(
                        buf,
                        x,
                        y,
                        &after,
                        base_style,
                        msg_width.saturating_sub(x - MSG_CONTENT_X),
                    );
                }
            } else {
                draw_str(buf, MSG_CONTENT_X, y, &line.text, base_style, msg_width);
            }
        } else {
            draw_str(buf, MSG_CONTENT_X, y, &line.text, base_style, msg_width);
        }
    }

    let input_row = size.height;
    let input_y = input_row - 1;
    fill_rect(
        buf,
        0,
        input_y,
        size.width,
        1,
        Style::default().bg(COLOR_STEP3),
    );
    let input_border = if app.active_turn_id.is_some() {
        COLOR_STEP8
    } else {
        COLOR_STEP7
    };
    draw_str(buf, 0, input_y, "┃", Style::default().fg(input_border), 1);

    let marker = if app.active_turn_id.is_some() {
        "» "
    } else {
        "› "
    };
    let marker_w = visual_width(marker);
    draw_str(
        buf,
        MSG_CONTENT_X,
        input_y,
        marker,
        Style::default()
            .fg(COLOR_GUTTER_USER)
            .add_modifier(Modifier::BOLD),
        transcript_content_width(size),
    );

    let input_w = transcript_content_width(size);
    let available = input_w.saturating_sub(marker_w);
    let input_tail_idx = if visual_width(&app.input) > available {
        let full = visual_width(&app.input);
        let skip_cells = full.saturating_sub(available);
        split_at_cells(&app.input, skip_cells)
    } else {
        0
    };
    let input_view = &app.input[input_tail_idx..];

    draw_str(
        buf,
        MSG_CONTENT_X + marker_w,
        input_y,
        input_view,
        Style::default().fg(COLOR_TEXT),
        available,
    );

    if app.show_help {
        draw_help_overlay(buf, size);
    }

    let mut cursor_x = MSG_CONTENT_X + marker_w + visual_width(input_view);
    let max_cursor_x = size.width.saturating_sub(1);
    if cursor_x > max_cursor_x {
        cursor_x = max_cursor_x;
    }
    frame.set_cursor_position((cursor_x as u16, input_y as u16));
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
                                    let copied =
                                        selected_text(sel, &rendered, size, app.scroll_top);
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
                            (KeyCode::Home, _) => {
                                app.auto_follow_bottom = false;
                                app.scroll_top = 0;
                            }
                            (KeyCode::End, _) => {
                                app.auto_follow_bottom = true;
                            }
                            (KeyCode::Up, _) => {
                                app.auto_follow_bottom = false;
                                app.scroll_top = app.scroll_top.saturating_sub(1);
                            }
                            (KeyCode::Down, _) => {
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
                            (KeyCode::Backspace, _) => {
                                app.input.pop();
                            }
                            (KeyCode::Enter, _) => {
                                if app.input.is_empty() {
                                    continue;
                                }

                                let text = std::mem::take(&mut app.input);
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
                                if app.input.is_empty() {
                                    app.show_help = true;
                                } else {
                                    app.input.push('?');
                                }
                            }
                            (KeyCode::Char('g'), _) if app.input.is_empty() => {
                                app.auto_follow_bottom = false;
                                app.scroll_top = 0;
                            }
                            (KeyCode::Char('G'), _) if app.input.is_empty() => {
                                app.auto_follow_bottom = true;
                            }
                            (KeyCode::Char(ch), mods)
                                if mods == KeyModifiers::NONE || mods == KeyModifiers::SHIFT =>
                            {
                                app.input.push(ch);
                            }
                            _ => {}
                        }
                    }
                    Event::Mouse(m) => {
                        if app.show_help {
                            continue;
                        }

                        let msg_top = MSG_TOP;
                        let msg_bottom = size.height.saturating_sub(1);
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

                                        let copied =
                                            selected_text(*sel, &rendered, size, app.scroll_top);
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

        let out = selected_text(
            sel,
            &lines,
            TerminalSize {
                width: 80,
                height: 20,
            },
            0,
        );
        assert_eq!(out, "  he");
    }

    #[test]
    fn selected_text_joins_soft_wrapped_rows_without_newline() {
        let lines = vec![
            RenderedLine {
                text: "abcde".to_string(),
                role: Role::Assistant,
                separator: false,
                cells: 5,
                soft_wrap_to_next: true,
            },
            RenderedLine {
                text: "fghij".to_string(),
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

        let out = selected_text(
            sel,
            &lines,
            TerminalSize {
                width: 80,
                height: 20,
            },
            0,
        );
        assert_eq!(out, "abcdefghij");
    }

    #[test]
    fn selected_text_keeps_newline_on_hard_break_rows() {
        let lines = vec![
            RenderedLine {
                text: "abcde".to_string(),
                role: Role::Assistant,
                separator: false,
                cells: 5,
                soft_wrap_to_next: false,
            },
            RenderedLine {
                text: "fghij".to_string(),
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

        let out = selected_text(
            sel,
            &lines,
            TerminalSize {
                width: 80,
                height: 20,
            },
            0,
        );
        assert_eq!(out, "abcde\nfghij");
    }

    #[test]
    fn build_rendered_lines_inserts_separator_rows_between_messages() {
        let messages = vec![
            Message {
                role: Role::User,
                text: "first".to_string(),
            },
            Message {
                role: Role::Assistant,
                text: "second".to_string(),
            },
        ];

        let rendered = build_rendered_lines(&messages, 40);
        assert!(rendered.len() >= 3);
        assert!(rendered[1].separator);
        assert_eq!(rendered[1].role, Role::System);
    }

    #[test]
    fn build_rendered_lines_hides_markdown_fence_delimiters() {
        let messages = vec![Message {
            role: Role::Assistant,
            text: "```zig\nconst x = 1;\n```\n".to_string(),
        }];

        let rendered = build_rendered_lines(&messages, 60);
        assert!(rendered.iter().all(|l| !l.text.contains("```")));
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
