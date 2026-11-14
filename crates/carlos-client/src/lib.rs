//! Share checked wire data and session copies while platforms own sockets, clocks and rendering.

mod bodies;
mod body_apply;
mod body_pieces;
mod body_views;
mod cards;
pub mod codec;
mod connection;
mod error;
mod goal;
mod history;
mod operations;
mod replica;
mod rows;

pub use bodies::{Bodies, BodyToken, OutputWindow, SourceWindow};
pub use connection::{Connection, ConnectionId, Welcome};
pub use error::ClientError;
pub use operations::{LocalOperation, LocalText, Operations};
pub use replica::{ChangeSet, Replica};

#[cfg(test)]
mod tests;
