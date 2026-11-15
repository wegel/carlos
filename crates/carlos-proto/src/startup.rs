//! Typed saved-session choices and first-launch options; native addresses never leave the daemon.

use serde::{Deserialize, Serialize};

use crate::ids::{SavedChoiceId, SavedScopeId};
use crate::session::Backend;

/// Preserve the terminal's explicit Codex summary choice without exposing backend JSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningSummary {
    Auto,
    Concise,
    Detailed,
    None,
}

/// Apply environment choices only when opening a native process, never when attaching an owner.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeLaunchOptions {
    /// Constrain the daemon's single configured Codex endpoint; omission accepts its default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_socket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_summary: Option<ReasoningSummary>,
    /// Preserve an explicit raw Unicode child value, including empty to clear an inherited value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_context_window: Option<String>,
}

impl NativeLaunchOptions {
    /// Omit an empty first-launch group so old create envelopes stay unchanged.
    pub fn is_empty(&self) -> bool {
        self.codex_socket.is_none()
            && self.reasoning_summary.is_none()
            && self.claude_context_window.is_none()
    }
}

impl ReasoningSummary {
    /// Return only one existing backend setting spelling.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Concise => "concise",
            Self::Detailed => "detailed",
            Self::None => "none",
        }
    }
}

/// Keep a continue lookup distinct from browsing; Grok uses one native candidate and no local fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedPurpose {
    Picker,
    Continue,
}

/// Navigate within one private picker without sending a native backend cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SavedDirection {
    Newest,
    Older,
    Newer,
    Oldest,
}

/// Select an earlier opaque picker choice or import one explicit legacy address at this boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OpenTarget {
    Resume {
        address: String,
    },
    Choice {
        scope: SavedScopeId,
        choice: SavedChoiceId,
    },
}

/// Plain bounded metadata for one row; complete original session text becomes available on open.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedChoice {
    pub choice: SavedChoiceId,
    pub name: Option<String>,
    pub preview: String,
    pub cwd: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// One private 32-row page whose choices expire when its peer closes or replaces the page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedPage {
    pub scope: SavedScopeId,
    pub backend: Backend,
    pub rows: Vec<SavedChoice>,
    /// Offer another picker page; continue lookups never offer page navigation.
    pub has_newer: bool,
    pub has_older: bool,
    pub can_archive: bool,
}

#[cfg(test)]
#[path = "startup_tests.rs"]
mod tests;
