//! Type definitions, constants, and small helpers for the Claude CLI backend.

use std::collections::{HashMap, HashSet};
use std::env;
use std::path::PathBuf;

use serde_json::{json, Map, Value};

// --- Types ---

#[derive(Debug, Clone)]
pub(crate) enum ClaudeLaunchMode {
    New,
    Resume(String),
}

#[derive(Debug, Clone)]
pub(crate) struct ClaudeToolCall {
    pub(super) name: String,
    pub(super) input: Value,
}

#[derive(Debug, Clone)]
pub(crate) enum ClaudeBlockState {
    Thinking {
        item_id: String,
        text: String,
    },
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
    pub(super) session_id: Option<String>,
    pub(crate) stream_instance_seq: u64,
    pub(super) next_turn_seq: u64,
    pub(crate) current_turn_id: Option<String>,
    pub(super) current_message_seq: u64,
    pub(super) current_message_id: Option<String>,
    pub(super) current_message_has_content_blocks: bool,
    pub(super) current_blocks: HashMap<usize, ClaudeBlockState>,
    pub(super) streamed_blocks: HashMap<usize, String>,
    pub(super) snapshot_keys: HashSet<String>,
    pub(super) next_snapshot_block: usize,
    pub(super) tool_calls: HashMap<String, ClaudeToolCall>,
    pub(super) interrupt_requested: bool,
}

#[derive(Debug, Default)]
pub(crate) struct TranslateOutput {
    pub(crate) lines: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ClaudeRuntimeSettings {
    pub(super) model: Option<String>,
    pub(super) effort: Option<String>,
}

// --- Turn management helpers ---

pub(super) fn ensure_claude_turn_started(
    state: &mut ClaudeTranslationState,
    out: &mut TranslateOutput,
) {
    if state.current_turn_id.is_some() {
        return;
    }

    state.next_turn_seq = state.next_turn_seq.saturating_add(1);
    let turn_id = claude_turn_item_id(state, state.next_turn_seq);
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

pub(super) fn begin_claude_message(state: &mut ClaudeTranslationState, message_id: Option<&str>) {
    state.current_message_seq = state.current_message_seq.saturating_add(1);
    state.current_message_id = message_id
        .map(str::trim)
        .filter(|message_id| !message_id.is_empty())
        .map(ToOwned::to_owned);
    state.current_message_has_content_blocks = false;
    state.current_blocks.clear();
    state.streamed_blocks.clear();
    state.snapshot_keys.clear();
    state.next_snapshot_block = 0;
}

pub(super) fn claude_message_item_id(state: &ClaudeTranslationState, index: usize) -> String {
    format!(
        "claude-msg-{}-{}-{}",
        state.stream_instance_seq, state.current_message_seq, index
    )
}

pub(super) fn claude_tool_item_id(state: &ClaudeTranslationState, index: usize) -> String {
    format!(
        "claude-tool-{}-{}-{}",
        state.stream_instance_seq, state.current_message_seq, index
    )
}

pub(super) fn claude_turn_item_id(state: &ClaudeTranslationState, turn_seq: u64) -> String {
    format!("claude-turn-{}-{}", state.stream_instance_seq, turn_seq)
}

// --- Path helpers ---

pub(crate) fn claude_config_dir() -> Option<PathBuf> {
    env::var("CLAUDE_CONFIG_DIR")
        .ok()
        .and_then(|value| normalize_claude_config_dir(&value))
        .or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .map(|home| home.join(".claude"))
        })
}

fn normalize_claude_config_dir(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(expand_home_path(trimmed))
}

fn expand_home_path(raw: &str) -> PathBuf {
    if raw == "~" {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(raw));
    }
    if let Some(stripped) = raw.strip_prefix("~/") {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(stripped))
            .unwrap_or_else(|| PathBuf::from(raw));
    }
    PathBuf::from(raw)
}

pub(super) fn normalize_runtime_arg(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("none"))
        .map(ToOwned::to_owned)
}

// --- Transcript filtering ---

pub(super) fn should_hide_claude_tool_transcript(tool_name: &str, input: &Value) -> bool {
    let lower = tool_name.trim().to_ascii_lowercase();
    match lower.as_str() {
        "agent" | "enterplanmode" | "enterworktree" | "exitplanmode" | "exitworktree"
        | "toolsearch" | "todowrite" => true,
        "write" => input
            .get("file_path")
            .or_else(|| input.get("filePath"))
            .and_then(Value::as_str)
            .is_some_and(is_claude_plan_artifact_path),
        _ => false,
    }
}

fn is_claude_plan_artifact_path(path: &str) -> bool {
    let normalized = path.trim().replace('\\', "/");
    if let Some(config_dir) = claude_config_dir() {
        let config_plans_prefix = format!(
            "{}/plans/",
            config_dir
                .to_string_lossy()
                .replace('\\', "/")
                .trim_end_matches('/')
        );
        if normalized.starts_with(&config_plans_prefix) {
            return true;
        }
    }
    normalized.contains("/.claude/plans/")
        || normalized.starts_with(".claude/plans/")
        || normalized.starts_with("~/.claude/plans/")
}

// --- Translation utility helpers ---

pub(super) fn parse_partial_json_object(input_json: &str) -> Map<String, Value> {
    if input_json.trim().is_empty() {
        return Map::new();
    }
    serde_json::from_str::<Map<String, Value>>(input_json).unwrap_or_default()
}

impl ClaudeTranslationState {
    /// Start a complete historical assistant snapshot without sharing live stream cursors.
    pub(crate) fn begin_history_snapshot(&mut self) {
        begin_claude_message(self, None);
    }
}
