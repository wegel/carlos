//! Keep reconnect generations and charged replay frames separate from the composer.

use std::collections::VecDeque;
use std::path::PathBuf;

use serde_json::Value;

#[derive(Default)]
/// One native connection may rebuild while earlier generations still drain to the UI.
pub(super) struct ConnectionState {
    pub(super) generation: u64,
    pub(super) closed_generation: Option<u64>,
    pub(super) unreplayed_children: usize,
    pub(super) runtime_read: Option<MetadataRead>,
    pub(super) name_revision: u64,
    pub(super) runtime_refresh: bool,
    pub(super) initial_rebuild: bool,
    pub(super) initial_choices: Option<Value>,
    pub(super) streams: super::codex_reconnect_streams::Streams,
    pub(super) socket: Option<PathBuf>,
    pub(super) rebuilding: bool,
    pub(super) journal: VecDeque<ReplayFrame>,
    pub(super) bytes: usize,
}

/// Retain the original envelope charge until this native event has actually been handled.
pub(super) struct ReplayFrame {
    pub(super) event: Value,
    pub(super) charged: usize,
}

/// Reconcile names and model settings independently when either changes during a read.
#[derive(Clone, Copy)]
pub(super) struct MetadataRead {
    pub(super) token: u64,
    pub(super) settings_revision: u64,
    pub(super) name_revision: u64,
}
