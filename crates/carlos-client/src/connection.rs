//! Retain private retry rights across socket reconnects without owning a socket or resending work.

use carlos_proto::commands::{Command, HelloResult};
use carlos_proto::ids::{ClientId, DaemonId, OperationId, ResumeToken};
use carlos_proto::submissions::ClientOperation;

use crate::ClientError;

/// Platforms attach this number to received messages so old sockets cannot update a new connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionId(u64);

/// A new lease can use the same daemon while losing rights to an expired client's old operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Welcome {
    Fresh,
    Resumed,
    Replaced { instance_changed: bool },
}

#[derive(Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Disconnected,
    Greeting,
    Ready,
}

struct Credential {
    client: ClientId,
    instance: DaemonId,
    token: ResumeToken,
    next: Option<OperationId>,
}

/// One device's private identity and monotonic counters; deliberately omit Debug to protect tokens.
#[derive(Default)]
pub struct Connection {
    phase: Phase,
    generation: u64,
    credential: Option<Credential>,
}

impl Connection {
    /// Begin one fresh transport generation and offer existing private retry rights, if any.
    pub fn begin(&mut self, name: String) -> Result<(ConnectionId, Command), ClientError> {
        if self.phase != Phase::Disconnected || name.len() > 256 {
            return Err(ClientError::InvalidConnection);
        }
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(ClientError::Capacity("client connection counter exhausted"))?;
        self.generation = generation;
        self.phase = Phase::Greeting;
        let old = self.credential.as_ref();
        Ok((
            ConnectionId(generation),
            Command::Hello {
                protocol: carlos_proto::PROTOCOL_VERSION,
                client: name,
                token: String::new(),
                resume: old.map(|value| value.client.clone()),
                resume_token: old.map(|value| value.token.clone()),
                instance: old.map(|value| value.instance.clone()),
            },
        ))
    }

    /// Accept a checked greeting; a resumed lease must retain its exact identity, instance and token.
    pub fn accept(
        &mut self,
        generation: ConnectionId,
        hello: HelloResult,
    ) -> Result<Welcome, ClientError> {
        if generation.0 != self.generation || self.phase != Phase::Greeting {
            return Err(ClientError::InvalidConnection);
        }
        if hello.protocol != carlos_proto::PROTOCOL_VERSION {
            return Err(ClientError::InvalidPayload(
                "daemon protocol differs from this client".into(),
            ));
        }
        let welcome = if hello.resumed {
            let old = self
                .credential
                .as_ref()
                .ok_or(ClientError::InvalidConnection)?;
            if old.client != hello.client
                || old.instance != hello.instance
                || old.token != hello.resume_token
            {
                return Err(ClientError::InvalidConnection);
            }
            Welcome::Resumed
        } else {
            match &self.credential {
                None => Welcome::Fresh,
                Some(old) => Welcome::Replaced {
                    instance_changed: old.instance != hello.instance,
                },
            }
        };
        let next = if welcome == Welcome::Resumed {
            // None means exhaustion, rather than zero or an instruction to reuse an old counter.
            self.credential
                .as_ref()
                .expect("resumed credential")
                .next
                .zip(hello.next_operation)
                .map(|(local, remote)| local.max(remote))
        } else {
            hello.next_operation
        };
        self.credential = Some(Credential {
            client: hello.client,
            instance: hello.instance,
            token: hello.resume_token,
            next,
        });
        self.phase = Phase::Ready;
        Ok(welcome)
    }

    /// Keep credentials after EOF, but do not grant new operations while the transport is down.
    pub fn disconnect(&mut self, generation: ConnectionId) {
        if generation.0 == self.generation {
            self.phase = Phase::Disconnected;
        }
    }

    /// Reject frames and write callbacks from an older or ungreeted transport.
    pub fn is_ready(&self, generation: ConnectionId) -> bool {
        generation.0 == self.generation && self.phase == Phase::Ready
    }

    /// Borrow the public client id; private resume credentials never enter shared facts.
    pub fn client(&self) -> Option<&ClientId> {
        self.credential.as_ref().map(|value| &value.client)
    }

    /// Allocate one counter for explicit new work; reconnect never allocates or sends an operation.
    pub fn next_operation(
        &mut self,
        generation: ConnectionId,
    ) -> Result<ClientOperation, ClientError> {
        if !self.is_ready(generation) {
            return Err(ClientError::InvalidConnection);
        }
        let credential = self.credential.as_mut().expect("ready credential");
        let id = credential
            .next
            .ok_or(ClientError::Capacity("client operation counter exhausted"))?;
        credential.next = id.get().checked_add(1).and_then(OperationId::new);
        Ok(ClientOperation {
            client: credential.client.clone(),
            id,
        })
    }
}
