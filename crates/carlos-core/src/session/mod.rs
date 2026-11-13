//! Plain session facts, independent of terminal widgets and rendered rows.

pub mod entry_id;
pub mod entry_key;
pub mod history_identity;
pub mod models;
pub mod transcript;

#[cfg(test)]
mod session_headless_tests;

pub mod approval_reply;
pub mod approval_state;
pub mod claude;
pub mod codex;
pub mod context_usage;
mod control_answers;
mod control_liveness;
pub mod control_work;
mod item_sources;
mod item_updates;
mod items;
pub mod ralph;
mod ralph_commands;
pub mod ralph_runtime_state;
pub mod read_summary;
pub mod reasoning;
pub mod state;
mod state_commands;
mod submission_identity;
mod thread;
mod transcript_commands;
pub mod transcript_updates;
mod turns;

#[cfg(test)]
mod session_update_tests;
#[cfg(test)]
mod turn_boundary_tests;

pub mod runtime_settings;
mod runtime_settings_updates;

pub mod model_info;

pub mod input_history;
mod input_history_commands;
pub(crate) mod input_history_native;

pub mod shell_command;
pub mod shell_directory;
pub mod shell_read;
pub(crate) mod shell_search;
pub(crate) mod shell_words;
pub mod summary_request;
pub mod tool_command_view;
pub(crate) mod tool_diff;
pub mod tool_diff_summary;
pub(crate) mod tool_format;
mod tool_output_items;
pub mod tool_shell;
pub mod tool_source;
pub mod tools;

mod summary_state;
mod summary_updates;

#[cfg(test)]
mod tool_label_parity_tests;

#[cfg(test)]
mod summary_binding_tests;

pub mod rate_limits;
pub mod usage;
mod usage_updates;

pub mod questions;
pub mod text;

#[cfg(test)]
mod usage_tests;

#[cfg(test)]
pub(crate) mod test_backend;

pub mod item_changes;

pub(crate) mod item_receive;

pub mod item_history_draft;

pub(crate) mod native_activity;

pub(crate) mod notification_items;

pub(crate) mod notification_tools;

mod history_receive;
pub mod item_history;

mod item_deltas;

#[cfg(test)]
mod native_item_headless_tests;

pub mod actions;

pub(crate) mod automatic_toggle;

pub mod common_update;

pub(crate) mod common_notifications;

pub(crate) mod context_parse;

pub mod thread_commands;

pub mod thread_list;

pub(crate) mod notification_turns;

mod backend_actions;
mod backend_receive;
pub mod backend_update;

#[cfg(test)]
mod backend_receiver_tests;
#[cfg(test)]
mod backend_test_support;

mod name_commands;
pub mod prompt_commands;
mod prompt_settings;
mod queue_dispatch;
mod rewind_commands;
mod rewind_history;
mod turn_interrupt;

mod backend_deadlines;
#[cfg(test)]
mod prompt_command_tests;

mod event_approval;
mod event_metadata;
mod event_reads;
mod event_rows;
pub mod events;
mod transcript_journal;

#[cfg(test)]
mod event_tests;

#[cfg(test)]
mod event_input_tests;

pub mod output;
mod output_cursor;
mod output_normalize;
mod transcript_output;

#[cfg(test)]
mod output_tests;

#[cfg(test)]
mod output_client_tests;

mod command_guard;
pub mod command_work;
mod prompt_settle;
mod rewind_settle;

#[cfg(test)]
mod command_work_tests;

#[cfg(test)]
mod output_control_tests;

#[cfg(test)]
mod submission_identity_tests;

#[cfg(test)]
mod control_work_tests;
