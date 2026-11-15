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
mod control_execute;
mod control_liveness;
mod control_query;
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
pub mod runtime_startup;
pub mod runtime_startup_catalog;
pub(crate) mod settings_changes;
#[cfg(test)]
mod settings_changes_tests;
mod settings_publication;
#[cfg(test)]
mod settings_publication_tests;
mod settings_reads;

pub mod model_info;

pub mod input_history;
mod input_history_commands;
pub(crate) mod input_history_native;
pub mod prompt_reads;

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
mod common_native_tests;
#[cfg(test)]
mod native_item_headless_tests;
#[cfg(test)]
mod native_translation_tests;

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
mod event_goal;
#[cfg(test)]
mod event_goal_tests;
mod event_metadata;
mod event_queue;
#[cfg(test)]
mod event_queue_tests;
mod event_reads;
mod event_rows;
pub mod events;
pub(crate) mod perf_rows;
mod queue_preview;
mod status_publication;
pub mod task_activity;
mod task_activity_reads;
#[cfg(test)]
mod task_activity_tests;
pub mod task_activity_text;
#[cfg(test)]
mod task_activity_text_tests;
mod task_activity_views;
mod task_digest;
mod task_fields;
mod task_overview;
mod task_publication;
mod task_rows;
mod task_runs;
mod task_stop_claims;
pub mod task_stop_reply;
#[cfg(test)]
mod task_stop_reply_tests;
mod task_stop_target;
#[cfg(test)]
mod task_stop_tests;
mod task_stop_work;
#[cfg(test)]
mod task_view_tests;
mod task_views;
mod task_window_bytes;
mod task_window_receive;
mod task_window_rows;
mod task_window_state;
#[cfg(test)]
mod task_window_tests;
pub mod task_windows;
mod transcript_journal;

#[cfg(test)]
mod event_tests;

#[cfg(test)]
mod event_format_tests;
#[cfg(test)]
mod event_input_tests;

pub mod output;
mod output_cursor;
mod output_normalize;
#[cfg(test)]
mod rewind_owner_tests;
pub mod source;
#[cfg(test)]
mod source_tests;
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
mod approval_metadata_tests;
#[cfg(test)]
mod control_question_tests;
#[cfg(test)]
mod control_work_tests;

#[path = "../engine/mod.rs"]
pub mod engine;

mod grok_startup;

#[cfg(test)]
mod runtime_behavior_tests;

#[cfg(test)]
mod approval_allocation_tests;
#[cfg(test)]
mod approval_control_test_support;

#[cfg(test)]
mod input_contract_test_support;
#[cfg(test)]
mod input_contract_tests;

#[cfg(test)]
mod approval_frozen_inputs;
#[cfg(test)]
mod approval_frozen_oracle;
#[cfg(test)]
mod approval_operation_sequences;

#[cfg(test)]
mod grok_submission_tests;

#[cfg(test)]
mod claude_submission_tests;

#[cfg(test)]
mod claude_reducer_tests;

#[cfg(test)]
mod claude_receiver_tests;

#[cfg(test)]
mod claude_prompt_tests;

#[cfg(test)]
mod claude_thinking_tests;

#[cfg(test)]
mod prompt_echo_race_tests;
