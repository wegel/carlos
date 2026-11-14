//! Share checked wire data and session copies while platforms own sockets, clocks and rendering.

mod cards;
pub mod codec;
mod error;
mod history;
mod replica;
mod rows;

pub use error::ClientError;
pub use replica::{ChangeSet, Replica};

#[cfg(test)]
mod tests;
