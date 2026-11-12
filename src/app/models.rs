//! Temporary read-only bridge to the plain session model types.

pub(super) use crate::session::models::{Message, MessageKind, Role, ThreadSummary, TurnStatus};

#[cfg(test)]
pub(super) use crate::session::models::ToolOutputDetails;
