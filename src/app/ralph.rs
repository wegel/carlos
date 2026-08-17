//! Ralph-mode configuration, prompt loading, and turn-marker detection.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use super::{Message, Role};

// --- Ralph Defaults ---

pub(crate) const DEFAULT_PROMPT_PATH: &str = ".agents/ralph-prompt.md";
pub(crate) const DEFAULT_DONE_MARKER: &str = "@@COMPLETE@@";
pub(crate) const DEFAULT_BLOCKED_MARKER: &str = "@@BLOCKED@@";
pub(crate) const DEFAULT_WAITING_MARKER: &str = "@@AWAITING_HUMAN@@";
pub(crate) const DEFAULT_CONTINUATION_PROMPT: &str = concat!(
    "Ralph continuation: your previous turn ended without a stop marker.\n",
    "Resume only the current incomplete ExecPlan. Do not start another ExecPlan.\n\n",
    "Keep working unless you reached one of these states:\n",
    "- Human validation, completion, or landing gate: output @@AWAITING_HUMAN@@\n",
    "- Hard blocker: output @@BLOCKED@@\n",
    "- Every ExecPlan is complete and archived: output @@COMPLETE@@\n\n",
    "Put the marker on its own line. Never stop without a marker."
);

// --- Ralph Types ---

#[derive(Debug, Clone)]
pub(super) struct RalphConfig {
    pub(super) prompt_path: PathBuf,
    pub(super) base_prompt: String,
    pub(super) done_marker: String,
    pub(super) blocked_marker: String,
    pub(super) waiting_marker: String,
    pub(super) continuation_prompt: String,
}

#[derive(Debug, Clone)]
pub(super) struct RalphState {
    pub(super) config: RalphConfig,
    pub(super) primed: bool,
    pub(super) completed: bool,
    pub(super) waiting_for_user: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RalphTurnMarkers {
    pub(super) blocked: bool,
    pub(super) completed: bool,
    pub(super) waiting: bool,
}

impl RalphState {
    pub(super) fn new(config: RalphConfig) -> Self {
        Self {
            config,
            primed: false,
            completed: false,
            waiting_for_user: false,
        }
    }
}

// --- Config Loading ---

pub(super) fn load_ralph_config(
    cwd: &Path,
    prompt_path_override: Option<&str>,
    done_marker_override: Option<&str>,
    blocked_marker_override: Option<&str>,
    waiting_marker_override: Option<&str>,
) -> Result<RalphConfig> {
    let prompt_path = resolve_prompt_path(cwd, prompt_path_override);
    let base_prompt = fs::read_to_string(&prompt_path)
        .with_context(|| format!("failed to read Ralph prompt at {}", prompt_path.display()))?;
    let base_prompt = base_prompt.trim().to_string();
    if base_prompt.is_empty() {
        bail!("Ralph prompt file is empty: {}", prompt_path.display());
    }

    let done_marker =
        normalize_marker(done_marker_override.unwrap_or(DEFAULT_DONE_MARKER), "done")?;
    let blocked_marker = normalize_marker(
        blocked_marker_override.unwrap_or(DEFAULT_BLOCKED_MARKER),
        "blocked",
    )?;
    let waiting_marker = normalize_marker(
        waiting_marker_override.unwrap_or(DEFAULT_WAITING_MARKER),
        "waiting",
    )?;

    Ok(RalphConfig {
        prompt_path,
        base_prompt,
        done_marker,
        blocked_marker,
        waiting_marker,
        continuation_prompt: DEFAULT_CONTINUATION_PROMPT.to_string(),
    })
}

// --- Marker Detection ---

pub(super) fn detect_turn_markers(
    messages: &[Message],
    start_idx: usize,
    done_marker: &str,
    blocked_marker: &str,
    waiting_marker: &str,
) -> RalphTurnMarkers {
    let done = done_marker.trim();
    let blocked = blocked_marker.trim();
    let waiting = waiting_marker.trim();
    let mut out = RalphTurnMarkers {
        blocked: false,
        completed: false,
        waiting: false,
    };
    if done.is_empty() && blocked.is_empty() && waiting.is_empty() {
        return out;
    }

    for msg in messages.iter().skip(start_idx) {
        if !matches!(msg.role, Role::Assistant | Role::Commentary) {
            continue;
        }
        if !done.is_empty() && text_contains_marker(&msg.text, done) {
            out.completed = true;
        }
        if !blocked.is_empty() && text_contains_marker(&msg.text, blocked) {
            out.blocked = true;
        }
        if !waiting.is_empty() && text_contains_marker(&msg.text, waiting) {
            out.waiting = true;
        }
        if out.completed && out.blocked && out.waiting {
            break;
        }
    }
    out
}

fn text_contains_marker(text: &str, marker: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == marker
            || trimmed
                .strip_prefix(marker)
                .is_some_and(|suffix| suffix.starts_with(" @@"))
    })
}

// --- Marker Helpers ---

fn resolve_prompt_path(cwd: &Path, prompt_path_override: Option<&str>) -> PathBuf {
    let candidate = prompt_path_override.unwrap_or(DEFAULT_PROMPT_PATH);
    let path = Path::new(candidate);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn normalize_marker(marker: &str, name: &str) -> Result<String> {
    let trimmed = marker.trim();
    if trimmed.is_empty() {
        bail!("Ralph {name} marker cannot be empty");
    }
    Ok(trimmed.to_string())
}
