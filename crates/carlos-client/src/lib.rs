//! Share checked wire data and session copies while platforms own sockets, clocks and rendering.

mod bodies;
mod body_apply;
mod body_pieces;
mod body_views;
mod cards;
mod catalog;
mod client;
mod client_budget;
mod client_connect;
mod client_events;
mod client_history;
mod client_panels;
mod client_replies;
mod client_requests;
mod client_subscriptions;
mod client_watches;
pub mod codec;
mod connection;
mod error;
mod goal;
mod goal_watches;
mod history;
mod operations;
mod pending;
mod queue;
mod replica;
mod rows;
mod tasks;
mod text_ranges;

pub use bodies::{Bodies, BodyToken, OutputWindow, SourceWindow};
pub use client::{Client, Effects, ReadReply, ReadResult, RequestFailure, SessionChanges};
pub use connection::{Connection, ConnectionId, Welcome};
pub use error::ClientError;
pub use operations::{LocalOperation, LocalText, Operations};
pub use pending::Outbound;
pub use replica::{ChangeSet, Replica};

#[cfg(test)]
mod tests;
