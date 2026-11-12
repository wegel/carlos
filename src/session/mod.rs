//! Plain session facts, independent of terminal widgets and rendered rows.

pub(crate) mod entry_id;
pub(crate) mod entry_key;
pub(crate) mod history_identity;
pub(crate) mod models;
pub(crate) mod transcript;

#[cfg(test)]
mod session_headless_tests;
