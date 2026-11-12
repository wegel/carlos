//! Plain session facts, independent of terminal widgets and rendered rows.

pub(crate) mod entry_id;
pub(crate) mod entry_key;
pub(crate) mod history_identity;
pub(crate) mod models;
pub(crate) mod transcript;

#[cfg(test)]
mod session_headless_tests;

pub(crate) mod approval_state;
pub(crate) mod context_usage;
mod item_sources;
mod item_updates;
mod items;
pub(crate) mod ralph;
mod ralph_commands;
pub(crate) mod ralph_runtime_state;
pub(crate) mod read_summary;
pub(crate) mod reasoning;
pub(crate) mod state;
mod state_commands;
mod thread;
mod transcript_commands;
pub(crate) mod transcript_updates;
mod turns;

#[cfg(test)]
mod session_update_tests;
#[cfg(test)]
mod turn_boundary_tests;

pub(crate) mod runtime_settings;
mod runtime_settings_updates;

pub(crate) mod model_info;

pub(crate) mod input_history;
mod input_history_commands;
pub(crate) mod input_history_native;
