//! Own Claude metadata and optional backend reads without terminal state.

mod context;
mod model_choices;
mod models;
pub(crate) mod quota;
mod quota_updates;
mod state;

pub(crate) use state::{ClaudePhase, ClaudeState};

#[cfg(test)]
mod test_support;
