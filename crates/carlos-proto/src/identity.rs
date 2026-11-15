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
    /// Pair a binary package version and source marker with this protocol version.
    pub fn new(version: &str, source: &str) -> Self {
        Self {
            protocol: crate::PROTOCOL_VERSION,
            version: version.into(),
            source: source.into(),
        }
    }
}

/// Describe the daemon's immutable build and optional external native Codex endpoint.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DaemonIdentity {
    pub build: BuildInfo,
    /// Name the daemon process so a user can stop a mismatched build explicitly.
    #[serde(default)]
    pub process: u32,
    /// Preserve Unix path bytes; None means the daemon starts its own Codex process.
    pub codex_socket: Option<Vec<u8>>,
}
