//! Temporary read-only bridge to the plain session model types.

pub(super) use crate::session::models::{Message, MessageKind, Role, ThreadSummary};

#[cfg(test)]
pub(super) use crate::session::models::ToolOutputDetails;

#[cfg(test)]
pub(super) use crate::session::models::TurnStatus;
