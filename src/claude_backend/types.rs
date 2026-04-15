//! Type definitions, constants, and small helpers for the Claude CLI backend.

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use serde_json::{json, Map, Value};

use crate::protocol_params::ModelInfo;

// --- Constants ---

pub(crate) const CLAUDE_CONTEXT_WINDOW: u64 = 1_000_000;
pub(crate) const CLAUDE_PENDING_THREAD_ID: &str = "claude-pending-session";
pub(crate) const CLAUDE_EXIT_PLAN_REQUEST_METHOD: &str = "claude/exitPlan/requestApproval";
pub(crate) const CLAUDE_EXIT_PLAN_FALLBACK_TEXT: &str = "Exit plan mode?";
pub(crate) const CLAUDE_SUPPORTED_EFFORTS: [&str; 4] = ["low", "medium", "high", "max"];

// --- Types ---

#[derive(Debug, Clone)]
pub(crate) enum ClaudeLaunchMode {
    New,
    Resume(String),
    Continue,
}

#[derive(Debug, Clone)]
pub(crate) struct ClaudeToolCall {
    pub(super) name: String,
    pub(super) input: Value,
}

#[derive(Debug, Clone)]
pub(crate) struct ClaudeAllowedPrompt {
    pub(super) prompt: String,
    pub(super) tool: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ClaudeExitPlanApproval {
    pub(super) tool_use_id: String,
    pub(super) plan: String,
    pub(super) plan_file_path: Option<String>,
    pub(super) allowed_prompts: Vec<ClaudeAllowedPrompt>,
}

#[derive(Debug, Clone)]
pub(crate) enum ClaudeBlockState {
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
    pub(super) model: Option<String>,
    pub(super) next_turn_seq: u64,
    pub(super) current_turn_id: Option<String>,
    pub(super) current_message_seq: u64,
    pub(super) current_message_id: Option<String>,
    pub(super) current_message_has_content_blocks: bool,
    pub(super) current_blocks: HashMap<usize, ClaudeBlockState>,
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

pub(super) fn begin_claude_message(state: &mut ClaudeTranslationState, message_id: Option<&str>) {
    state.current_message_seq = state.current_message_seq.saturating_add(1);
    state.current_message_id = message_id
        .map(str::trim)
        .filter(|message_id| !message_id.is_empty())
        .map(ToOwned::to_owned);
    state.current_message_has_content_blocks = false;
    state.current_blocks.clear();
}

// --- Model catalog ---

pub(crate) fn claude_model_catalog() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            model: "claude-opus-4-6".to_string(),
            display_name: "Claude Opus 4.6".to_string(),
            supported_efforts: CLAUDE_SUPPORTED_EFFORTS
                .iter()
                .map(|effort| (*effort).to_string())
                .collect(),
            default_effort: Some("medium".to_string()),
            is_default: true,
        },
        ModelInfo {
            model: "claude-sonnet-4-6".to_string(),
            display_name: "Claude Sonnet 4.6".to_string(),
            supported_efforts: CLAUDE_SUPPORTED_EFFORTS
                .iter()
                .map(|effort| (*effort).to_string())
                .collect(),
            default_effort: Some("medium".to_string()),
            is_default: false,
        },
        ModelInfo {
            model: "claude-haiku-4-5".to_string(),
            display_name: "Claude Haiku 4.5".to_string(),
            supported_efforts: CLAUDE_SUPPORTED_EFFORTS
                .iter()
                .map(|effort| (*effort).to_string())
                .collect(),
            default_effort: Some("medium".to_string()),
            is_default: false,
        },
    ]
}

// --- Path helpers ---

pub(crate) fn claude_config_dir() -> Option<PathBuf> {
    env::var("CLAUDE_CONFIG_DIR")
        .ok()
        .and_then(|value| normalize_claude_config_dir(&value))
        .or_else(|| env::var_os("HOME").map(PathBuf::from).map(|home| home.join(".claude")))
}

pub(crate) fn claude_projects_root() -> Option<PathBuf> {
    claude_config_dir().map(|dir| dir.join("projects"))
}

pub(crate) fn claude_project_dir_name(cwd: &std::path::Path) -> String {
    cwd.to_string_lossy()
        .chars()
        .map(|ch| if ch == '/' || ch == '\\' { '-' } else { ch })
        .collect()
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
        return env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(raw));
    }
    if let Some(stripped) = raw.strip_prefix("~/") {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(stripped))
            .unwrap_or_else(|| PathBuf::from(raw));
    }
    PathBuf::from(raw)
}

pub(crate) fn claude_recovery_launch_mode(
    launch_mode: &ClaudeLaunchMode,
    current_session_id: Option<&str>,
) -> ClaudeLaunchMode {
    match current_session_id
        .map(str::trim)
        .filter(|session_id| !session_id.is_empty())
    {
        Some(session_id) => ClaudeLaunchMode::Resume(session_id.to_string()),
        None => launch_mode.clone(),
    }
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
        "agent"
        | "enterplanmode"
        | "enterworktree"
        | "exitplanmode"
        | "exitworktree"
        | "toolsearch"
        | "todowrite" => true,
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
            config_dir.to_string_lossy().replace('\\', "/").trim_end_matches('/')
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

pub(super) fn normalize_claude_model_name(raw: &str) -> String {
    raw.split('[').next().unwrap_or(raw).trim().to_string()
}

pub(super) fn parse_partial_json_object(input_json: &str) -> Map<String, Value> {
    if input_json.trim().is_empty() {
        return Map::new();
    }
    serde_json::from_str::<Map<String, Value>>(input_json).unwrap_or_default()
}

pub(super) fn synthetic_token_usage_line(usage: &Map<String, Value>, model: Option<&str>) -> String {
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
