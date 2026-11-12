//! Own Codex callback facts while other Codex concerns await their later milestones.

pub(crate) mod approval_parsing;
pub(crate) mod callback_commands;
pub(crate) mod callback_questions;
pub(crate) mod callback_receive;
pub(crate) mod callback_reply;
pub(crate) mod callbacks;

pub(crate) mod queue;
pub(crate) mod queue_receive;
pub(crate) mod queue_state;

mod queue_access;

#[cfg(test)]
mod queue_test_support;

pub(crate) mod queue_edit;

#[cfg(test)]
mod queue_tests;

mod goal_access;
pub(crate) mod goal_receive;
pub(crate) mod goal_state;
pub(crate) mod goals;

pub(crate) mod scope;

#[cfg(test)]
mod goal_test_support;

#[cfg(test)]
mod goal_tests;

pub(crate) mod settings;
mod settings_access;
pub(crate) mod settings_reply;
pub(crate) mod settings_state;

#[cfg(test)]
mod settings_test_support;

#[cfg(test)]
mod settings_tests;

pub(crate) mod panel;
mod panel_access;
pub(crate) mod panel_activity;
pub(crate) mod panel_events;
pub(crate) mod panel_pages;
pub(crate) mod panel_receive;
pub(crate) mod panel_scope;
pub(crate) mod panel_state;
pub(crate) mod task_badge;
pub(crate) mod task_status;

pub(crate) mod panel_snapshot;

#[cfg(test)]
mod panel_test_support;

#[cfg(test)]
mod panel_snapshot_tests;

pub(in crate::session) mod metadata;

pub(crate) mod history;
