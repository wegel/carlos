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
