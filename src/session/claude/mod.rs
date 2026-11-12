//! Own Claude helper and process facts without terminal state.

mod activity;
mod browser;
mod compaction;
mod context;
mod model_choices;
mod models;
mod process;
pub(crate) mod quota;
mod quota_updates;
mod state;
mod task_commands;
mod task_reads;
mod task_stops;
mod tasks;

pub(crate) use activity::{ClaudeActivity, NativeState};
pub(crate) use compaction::CompactionResult;
pub(crate) use state::{ClaudePhase, ClaudeState};
pub(crate) use task_stops::StopState;
pub(crate) use tasks::{ClaudeTasks, TaskStatus};

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod stop_parity_tests;
