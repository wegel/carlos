//! Own Claude helper and process facts without terminal state.

mod activity;
mod browser;
pub mod callback_questions;
mod callback_reply;
pub mod callbacks;
mod compaction;
mod context;
mod edit;
mod events;
mod history;
mod model_choices;
mod models;
mod notices;
mod notifications;
mod process;
mod prompt_transcript;
mod prompts;
pub mod quota;
mod quota_reads;
mod quota_updates;
mod reset;
mod state;
mod task_commands;
mod task_reads;
mod task_stops;
mod tasks;

pub use activity::{ClaudeActivity, NativeState};
pub use compaction::CompactionResult;
pub use edit::EditProgress;
pub use notifications::ClaudeRoute;
pub(crate) use quota_reads::QuotaReadKey;
pub use state::{ClaudePhase, ClaudeState};
pub use task_stops::StopState;
pub use tasks::{ClaudeTasks, TaskStatus};

#[cfg(any(test, feature = "test-support"))]
mod test_support;

#[cfg(test)]
mod stop_parity_tests;

#[cfg(test)]
mod callback_tests;

mod receive_controls;
mod receive_named;
mod receive_recovery;
mod receive_reset;
mod receive_sdk;

#[cfg(test)]
mod receive_headless_tests;
pub mod receive_update;

#[cfg(test)]
mod quota_read_tests;

#[cfg(test)]
mod task_behavior_tests;
