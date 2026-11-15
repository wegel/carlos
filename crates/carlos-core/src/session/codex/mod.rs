//! Own Codex callback facts while other Codex concerns await their later milestones.

pub(crate) mod approval_parsing;
pub mod callback_commands;
pub mod callback_questions;
pub(crate) mod callback_receive;
pub mod callback_reply;
pub mod callbacks;

pub mod queue;
pub(crate) mod queue_receive;
pub mod queue_state;

mod queue_access;

#[cfg(any(test, feature = "test-support"))]
mod queue_test_support;

pub mod queue_edit;

#[cfg(test)]
mod queue_tests;

mod goal_access;
pub(crate) mod goal_receive;
pub mod goal_state;
pub mod goals;

pub mod scope;

#[cfg(any(test, feature = "test-support"))]
mod goal_test_support;

#[cfg(test)]
mod goal_tests;

pub mod settings;
mod settings_access;
mod settings_dispatch;
#[cfg(test)]
mod settings_owned_tests;
pub(crate) mod settings_reply;
pub(crate) mod settings_state;

#[cfg(any(test, feature = "test-support"))]
mod settings_test_support;

#[cfg(test)]
mod settings_tests;

pub mod panel;
mod panel_access;
pub(crate) mod panel_activity;
pub(crate) mod panel_events;
#[cfg(test)]
mod panel_interest_tests;
mod panel_lineage;
pub(crate) mod panel_pages;
pub(in crate::session) mod panel_process_runs;
pub(in crate::session) mod panel_reader;
#[cfg(test)]
mod panel_reader_tests;
mod panel_reads;
pub mod panel_receive;
#[cfg(test)]
mod panel_run_tests;
pub(in crate::session) mod panel_runs;
pub mod panel_scope;
pub mod panel_state;
pub mod task_badge;
pub mod task_status;

pub(crate) mod panel_snapshot;

#[cfg(any(test, feature = "test-support"))]
mod panel_test_support;

#[cfg(test)]
mod panel_snapshot_tests;

pub(in crate::session) mod metadata;

pub mod history;

pub mod connection_state;

pub(crate) mod connection_commands;

pub mod lifecycle;

pub mod reconnect_runtime;

pub mod reconnect_streams;

pub(crate) mod reconnect_replay;

pub(crate) mod reconnect_install;

#[cfg(any(test, feature = "test-support"))]
mod connection_test_support;

#[cfg(test)]
mod reconnect_headless_tests;

pub(crate) mod callback_route;
pub mod callback_update;

#[cfg(test)]
mod callback_route_tests;

pub(crate) mod frame_batch;
pub mod frame_install;
mod frame_receive;

#[cfg(test)]
mod frame_tests;
