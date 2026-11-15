//! Describe a host's saved next-launch context size separately from live token usage.

use serde::{Deserialize, Serialize};

use crate::ids::SessionId;

/// Smallest explicit context default accepted by the existing editor.
pub const MIN_CONTEXT_TOKENS: u64 = 100_000;
/// Largest explicit context default accepted by the existing editor.
pub const MAX_CONTEXT_TOKENS: u64 = 1_000_000;

/// Explain whether Carlos may change this backend's saved next-launch size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextPolicy {
    Owned,
    AttachedCodex,
    ClaudeShellOverride,
    Unsupported,
}

/// Require an explicit Auto or token choice instead of treating omitted input as a reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "tokens",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ContextChoice {
    Auto,
    Tokens(u64),
}

impl From<Option<u64>> for ContextChoice {
    fn from(tokens: Option<u64>) -> Self {
        tokens.map_or(Self::Auto, Self::Tokens)
    }
}

impl From<ContextChoice> for Option<u64> {
    fn from(choice: ContextChoice) -> Self {
        match choice {
            ContextChoice::Auto => None,
            ContextChoice::Tokens(tokens) => Some(tokens),
        }
    }
}

/// Return one host default and its editor policy without claiming the active model uses it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextDefaults {
    pub session: SessionId,
    pub saved_tokens: Option<u64>,
    pub policy: ContextPolicy,
}

#[cfg(test)]
#[path = "context_defaults_tests.rs"]
mod tests;
