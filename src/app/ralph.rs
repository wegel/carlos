//! Re-export Ralph configuration and marker checks for existing app tests.

#[cfg(test)]
pub(super) use crate::session::ralph::{
    detect_turn_markers, RalphConfig, DEFAULT_CONTINUATION_PROMPT,
};
