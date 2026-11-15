//! Compare declared protocol, package version and source inputs without a security claim.

use serde::{Deserialize, Serialize};

/// Identify executables built from the same sorted source, lock and helper inputs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BuildInfo {
    pub protocol: u32,
    pub version: String,
    pub source: String,
}

impl BuildInfo {
    /// Report this workspace build's compatibility marker, independent of its directory.
    pub fn current() -> Self {
        Self {
            protocol: crate::PROTOCOL_VERSION,
            version: env!("CARGO_PKG_VERSION").into(),
            source: env!("CARLOS_SOURCE_ID").into(),
        }
    }
}

/// Describe the daemon's immutable build and optional external native Codex endpoint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DaemonIdentity {
    pub build: BuildInfo,
    /// Preserve Unix path bytes; None means the daemon starts its own Codex process.
    pub codex_socket: Option<Vec<u8>>,
}
