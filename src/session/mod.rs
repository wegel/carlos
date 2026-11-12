//! Plain session facts, independent of terminal widgets and rendered rows.

pub(crate) mod entry_id;
pub(crate) mod entry_key;
pub(crate) mod history_identity;
pub(crate) mod models;
pub(crate) mod transcript;

#[cfg(test)]
mod session_headless_tests;

pub(crate) mod approval_reply;
pub(crate) mod approval_state;
pub(crate) mod claude;
pub(crate) mod codex;
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

pub(crate) mod shell_command;
pub(crate) mod shell_directory;
pub(crate) mod shell_read;
pub(crate) mod shell_search;
pub(crate) mod shell_words;
pub(crate) mod summary_request;
pub(crate) mod tool_command_view;
pub(crate) mod tool_diff;
pub(crate) mod tool_diff_summary;
pub(crate) mod tool_format;
mod tool_output_items;
pub(crate) mod tool_shell;
pub(crate) mod tool_source;
pub(crate) mod tools;

mod summary_state;
mod summary_updates;

#[cfg(test)]
mod tool_label_parity_tests;

#[cfg(test)]
mod summary_binding_tests;

pub(crate) mod rate_limits;
pub(crate) mod usage;
mod usage_updates;

pub(crate) mod questions;
pub(crate) mod text;

#[cfg(test)]
mod usage_tests;

#[cfg(test)]
pub(crate) mod test_backend;

pub(crate) mod item_changes;

pub(crate) mod item_receive;

pub(crate) mod item_history_draft;

pub(crate) mod native_activity;

pub(crate) mod notification_items;

pub(crate) mod notification_tools;

mod history_receive;
pub(crate) mod item_history;

mod item_deltas;

#[cfg(test)]
mod native_item_headless_tests;
