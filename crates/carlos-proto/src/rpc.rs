//! JSON-RPC envelopes and version checks, independent of Carlos command families.

use serde::{Deserialize, Serialize};

use crate::commands::Command;
use crate::PROTOCOL_VERSION;

/// Only JSON-RPC version 2.0 is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RpcVersion {
    #[serde(rename = "2.0")]
    V2,
}

/// String ids avoid numeric narrowing; numeric ids are restricted to signed 64-bit integers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
    Integer(i64),
}

/// A client request, including its response correlation id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: RpcVersion,
    pub id: RequestId,
    #[serde(flatten)]
    pub command: Command,
}

/// An explicit response result, without ambiguous result/error objects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Success<T> {
    pub jsonrpc: RpcVersion,
    pub id: RequestId,
    pub result: T,
}

/// A rejected command; an invalid envelope may have a null correlation id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Failure {
    pub jsonrpc: RpcVersion,
    pub id: Option<RequestId>,
    pub error: RpcError,
}

/// Error codes follow JSON-RPC for standard errors and use -320xx for Carlos failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

/// Reject a protocol mismatch before a subscription or command is accepted.
pub fn check_protocol(version: u32) -> Result<(), RpcError> {
    if version == PROTOCOL_VERSION {
        return Ok(());
    }
    Err(RpcError {
        code: -32001,
        message: format!("unsupported protocol {version}; expected {PROTOCOL_VERSION}"),
    })
}
