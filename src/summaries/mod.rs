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

/// Raw commands use their exact hash; normalized views also key their version and directory context.
pub(crate) type Key = [u8; 32];

/// Hash the exact command text without executing or normalizing it.
pub(crate) fn command_key(command: &str) -> Key {
    *blake3::hash(command.as_bytes()).as_bytes()
}

/// Keep normalized labels distinct by parser version, original command and effective/read directories.
pub(crate) fn presentation_key(raw: &str, location: &CommandLocation) -> Key {
    let bytes = serde_json::to_vec(&(
        "cd-prefix-v1",
        raw,
        &location.cwd,
        &location.read_root,
        &location.directory,
    ))
    .expect("command location serializes");
    *blake3::hash(&bytes).as_bytes()
}

/// Resolve scripts from their actual directory without enlarging the original read authority.
#[derive(Clone, Debug)]
pub(crate) struct CommandLocation {
    pub cwd: PathBuf,
    pub read_root: PathBuf,
    pub directory: Option<String>,
}

impl CommandLocation {
    /// Ordinary commands retain the original source workspace and no extra directory hint.
    pub fn at(cwd: PathBuf) -> Self {
        Self {
            read_root: cwd.clone(),
            cwd,
            directory: None,
        }
    }

    /// Check physical paths off the UI thread, including links that lead outside the old authority.
    pub fn sources_allowed(&self) -> bool {
        self.read_root
            .canonicalize()
            .ok()
            .zip(self.cwd.canonicalize().ok())
            .is_some_and(|(root, cwd)| cwd.starts_with(root))
    }
}

/// A lookup may generate a label only for a newly observed live command.
#[derive(Clone, Debug)]
pub(crate) struct Request {
    pub key: Key,
    pub command: String,
    pub location: CommandLocation,
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
