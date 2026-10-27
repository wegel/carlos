//! Describe selected commands off the UI thread and keep indexed local labels.

pub(crate) mod config;
pub(crate) mod eligibility;
mod process;
mod provider;
pub(crate) mod reader;
mod store;
mod worker;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};

pub(crate) use worker::SummaryService;

/// Stable content key; directories and model versions are not part of it.
pub(crate) type Key = [u8; 32];

/// Hash the exact command text without executing or normalizing it.
pub(crate) fn command_key(command: &str) -> Key {
    *blake3::hash(command.as_bytes()).as_bytes()
}

/// A lookup may generate a label only for a newly observed live command.
#[derive(Clone, Debug)]
pub(crate) struct Request {
    pub key: Key,
    pub command: String,
    pub cwd: PathBuf,
    pub generate: bool,
    pub entry: String,
    pub file_based: bool,
}

/// The worker returns content keys, never transcript row indices.
#[derive(Debug)]
pub(crate) enum SummaryEvent {
    Ready(Key, String, Option<String>),
    Failed(String, String),
}

/// Keep source fingerprints separate from command-only cache identity.
#[derive(Clone, Debug)]
pub(crate) struct Source {
    pub path: PathBuf,
    pub hash: Key,
}

/// A model result carries the files actually inspected, when any.
#[derive(Clone)]
pub(crate) struct Label {
    pub text: String,
    pub sources: Vec<Source>,
}

pub(super) struct Control {
    enabled: AtomicBool,
    stopped: AtomicBool,
    pid: AtomicU32,
    epoch: AtomicU64,
}

#[cfg(test)]
mod tests;
