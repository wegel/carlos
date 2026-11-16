//! Describe provisioned devices without publishing bearer secrets in operator replies.

use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ids::DeviceId;
use crate::rpc::RpcError;
use crate::submissions::ClientOperation;

/// Maximum provisioned devices, independently of live sockets.
pub const MAX_DEVICES: usize = 32;
/// Maximum UTF-8 bytes in a single-line device name.
pub const MAX_NAME_BYTES: usize = 256;

/// A first-hello bearer credential; empty means Unix peer credentials, and Debug hides its bytes.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AccessToken(String);

impl AccessToken {
    /// Borrow the token only to encode a greeting or authenticate its secret.
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// Distinguish a local peer-credential greeting from a device-token greeting.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Reject malformed credentials without including supplied secret bytes in the error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidAccessToken;

impl fmt::Display for InvalidAccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected a device id and a 32-byte lowercase hexadecimal secret")
    }
}
impl std::error::Error for InvalidAccessToken {}

impl TryFrom<String> for AccessToken {
    type Error = InvalidAccessToken;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Ok(Self(value));
        }
        let valid = value.len() == 97
            && value.split_once('.').is_some_and(|(id, secret)| {
                id.parse::<DeviceId>().is_ok() && canonical_verifier(secret)
            });
        if valid {
            Ok(Self(value))
        } else {
            Err(InvalidAccessToken)
        }
    }
}
impl FromStr for AccessToken {
    type Err = InvalidAccessToken;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.to_owned().try_into()
    }
}
impl From<AccessToken> for String {
    fn from(value: AccessToken) -> Self {
        value.0
    }
}
impl fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AccessToken([redacted])")
    }
}

/// Safe operator-visible metadata; this object contains neither a secret nor its verifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceInfo {
    pub id: DeviceId,
    pub name: String,
}

impl DeviceInfo {
    /// Require a bounded readable name before storing or displaying device metadata.
    pub fn validate(&self) -> Result<(), RpcError> {
        if self.name.trim().is_empty()
            || self.name.len() > MAX_NAME_BYTES
            || self.name.chars().any(char::is_control)
        {
            return Err(invalid(
                "device name must be one nonempty line of at most256 bytes",
            ));
        }
        Ok(())
    }
}

/// Provision one previously generated credential through the authenticated Unix operator only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceAdd {
    pub operation: ClientOperation,
    pub device: DeviceInfo,
    pub verifier: String,
}

impl DeviceAdd {
    /// Check metadata and a canonical BLAKE3 verifier without accepting a bearer secret.
    pub fn validate(&self) -> Result<(), RpcError> {
        self.device.validate()?;
        if !canonical_verifier(&self.verifier) {
            return Err(invalid(
                "device verifier must be32 bytes of lowercase hexadecimal",
            ));
        }
        Ok(())
    }
}

/// Remove one device's durable access and retry rights through the Unix operator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceRevoke {
    pub operation: ClientOperation,
    pub device: DeviceId,
}

/// List only safe device names and ids; check the bound before a client adopts it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Devices {
    pub devices: Vec<DeviceInfo>,
}

impl Devices {
    /// Reject excess or duplicate records before exposing an operator list.
    pub fn validate(&self) -> Result<(), RpcError> {
        if self.devices.len() > MAX_DEVICES {
            return Err(invalid("device list exceeds32 records"));
        }
        let mut ids = HashSet::new();
        for device in &self.devices {
            device.validate()?;
            if !ids.insert(&device.id) {
                return Err(invalid("device list contains duplicate ids"));
            }
        }
        Ok(())
    }
}

fn canonical_verifier(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid(message: &str) -> RpcError {
    RpcError {
        code: -32602,
        message: message.into(),
    }
}
