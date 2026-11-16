//! Fence socket callbacks with a device-local generation while Rust retains private reply rights.

use std::sync::{Arc, Mutex, MutexGuard};

use carlos_client::{codec, Client, ConnectionId};
use carlos_proto::devices::AccessToken;

use crate::{PhoneBatch, PhoneError, PhoneFailure, PhoneFrame};

#[derive(Default)]
struct State {
    client: Client,
    generation: u64,
    current: Option<ConnectionId>,
}

/// One host's shared client, with no socket, clock, thread or backend process.
#[derive(Default, uniffi::Object)]
pub struct PhoneClient {
    state: Mutex<State>,
}

#[uniffi::export]
impl PhoneClient {
    /// Create an empty device-local client for one configured host.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Begin a fresh socket generation using an explicitly provisioned device token.
    pub fn begin(&self, name: String, token: String) -> Result<PhoneFrame, PhoneError> {
        let token: AccessToken = token.parse().map_err(|_| PhoneError::InvalidInput {
            message: "Enter a provisioned Carlos device token".into(),
        })?;
        if token.is_empty() {
            return Err(PhoneError::InvalidInput {
                message: "A network connection requires a device token".into(),
            });
        }
        let mut state = self.lock()?;
        let generation = state
            .generation
            .checked_add(1)
            .ok_or_else(|| PhoneError::Capacity {
                message: "phone connection counter exhausted".into(),
            })?;
        let outgoing = state.client.begin_with_token(name, token)?;
        state.generation = generation;
        state.current = Some(outgoing.connection());
        PhoneFrame::from_outbound(generation, outgoing)
    }

    /// Apply only the current socket's message; stale callbacks cannot invalidate a new connection.
    pub fn receive(&self, generation: u64, message: Vec<u8>) -> Result<PhoneBatch, PhoneError> {
        let mut state = self.lock()?;
        let Some(connection) = state.connection(generation) else {
            return Ok(PhoneBatch::ignored(state.ready()));
        };
        let incoming = codec::decode(&message)?;
        let effects = state.client.receive(connection, incoming)?;
        let outgoing = effects
            .outgoing
            .into_iter()
            .map(|outgoing| PhoneFrame::from_outbound(generation, outgoing))
            .collect::<Result<_, _>>()?;
        let failures = effects
            .failures
            .into_iter()
            .map(|failure| PhoneFailure {
                code: failure.error.code,
                message: failure.error.message,
            })
            .collect();
        Ok(PhoneBatch {
            outgoing,
            ready: state.ready(),
            failures,
        })
    }

    /// Retire one original socket without discarding the client's private reconnect rights.
    pub fn disconnect(&self, generation: u64) -> Result<(), PhoneError> {
        let mut state = self.lock()?;
        if let Some(connection) = state.connection(generation) {
            state.client.disconnect(connection);
            state.current = None;
        }
        Ok(())
    }

    /// Return the shared protocol's maximum incoming message bytes to the native socket owner.
    pub fn maximum_message_bytes(&self) -> u64 {
        codec::MAX_FRAME_BYTES as u64
    }
}

impl PhoneClient {
    fn lock(&self) -> Result<MutexGuard<'_, State>, PhoneError> {
        self.state.lock().map_err(|_| PhoneError::Internal {
            message: "phone client is unavailable; retain private text and reconnect".into(),
        })
    }
}

impl State {
    fn connection(&self, generation: u64) -> Option<ConnectionId> {
        (self.generation == generation)
            .then_some(self.current)
            .flatten()
    }

    fn ready(&self) -> bool {
        self.current
            .is_some_and(|connection| self.client.is_ready(connection))
    }
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
