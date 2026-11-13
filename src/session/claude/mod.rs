//! Own Claude helper and process facts without terminal state.

mod activity;
mod browser;
pub(crate) mod callback_questions;
mod callback_reply;
pub(crate) mod callbacks;
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
pub(crate) mod quota;
mod quota_updates;
mod reset;
mod state;
mod task_commands;
mod task_reads;
mod task_stops;
mod tasks;

pub(crate) use activity::{ClaudeActivity, NativeState};
pub(crate) use compaction::CompactionResult;
pub(crate) use edit::EditProgress;
pub(crate) use notifications::ClaudeRoute;
pub(crate) use state::{ClaudePhase, ClaudeState};
pub(crate) use task_stops::StopState;
pub(crate) use tasks::{ClaudeTasks, TaskStatus};

#[cfg(test)]
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
pub(crate) mod receive_update;
