//! Keep reconnect generations and charged replay frames separate from the composer.

use std::collections::VecDeque;
use std::path::PathBuf;

use serde_json::Value;

/// One native connection may rebuild while earlier generations still drain to the UI.
#[derive(Debug, Default)]
pub struct ConnectionState {
    pub(in crate::session) generation: u64,
    pub(in crate::session) closed_generation: Option<u64>,
    pub(in crate::session) unreplayed_children: usize,
    pub(in crate::session) runtime_read: Option<MetadataRead>,
    pub(in crate::session) runtime_refresh: bool,
    pub(in crate::session) initial_rebuild: bool,
    pub(in crate::session) initial_choices: Option<Value>,
    pub(in crate::session) startup: Option<super::startup::StartupChoices>,
    pub(in crate::session) streams: super::reconnect_streams::Streams,
    pub(in crate::session) socket: Option<PathBuf>,
    pub(in crate::session) rebuilding: bool,
    pub(in crate::session) journal: VecDeque<ReplayFrame>,
    pub(in crate::session) bytes: usize,
}

/// Retain the original envelope charge until this native event has actually been handled.
#[derive(Debug)]
pub(crate) struct ReplayFrame {
    pub event: Value,
    pub charged: usize,
}

/// Reconcile names and model settings independently when either changes during a read.
#[derive(Clone, Copy, Debug)]
pub(in crate::session) struct MetadataRead {
    pub(in crate::session) token: u64,
    pub(in crate::session) settings_revision: u64,
    pub(in crate::session) name_revision: u64,
}

impl ConnectionState {
    /// Read the generation accepted by the current transport.
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Check whether native frames still await a complete reconnect snapshot.
    pub fn rebuilding(&self) -> bool {
        self.rebuilding
    }
    /// Borrow the configured native socket path.
    pub fn socket(&self) -> &Option<PathBuf> {
        &self.socket
    }
    /// Check whether a metadata refresh should run.
    pub fn runtime_refresh(&self) -> bool {
        self.runtime_refresh
    }
    /// Check whether an exact native metadata read remains in flight.
    pub(crate) fn runtime_read_pending(&self) -> bool {
        self.runtime_read.is_some()
    }
    #[cfg(any(test, feature = "test-support"))]
    /// Borrow the fixture child-agent ids still waiting for native replay.
    pub fn unreplayed_children(&self) -> usize {
        self.unreplayed_children
    }
}
