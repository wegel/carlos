//! Bind commands to one authenticated client and reject expired retries before native dispatch.

mod clients;
mod existing;

use std::collections::HashMap;
use std::time::Instant;

use carlos_proto::commands::{HelloResult, RpcError};
use carlos_proto::ids::{ClientId, DaemonId, ResumeToken, SessionId};
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState};

use self::clients::Clients;

/// A public client id needs its private credential and the current daemon instance to resume.
pub struct Resume {
    pub client: ClientId,
    pub secret: ResumeToken,
    pub instance: DaemonId,
}

/// Urgent operations retain space independently from ordinary sends and creates.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Ordinary,
    Control,
}

/// A retry returns its retained result; only a new admission may start native work.
pub enum Admission {
    New(OperationResult),
    Existing(OperationResult),
}

struct Window {
    count: usize,
    bytes: usize,
    count_limit: usize,
    byte_limit: usize,
}
struct Record {
    canonical: Vec<u8>,
    result: OperationResult,
    class: Class,
    reserved: usize,
    order: u64,
}

/// The manager owns this store after device sockets disappear; it persists no native jobs.
pub struct RetryStore {
    instance: DaemonId,
    clients: Clients,
    records: HashMap<ClientOperation, Record>,
    windows: [Window; 2],
    issued: u64,
}

impl RetryStore {
    /// Start fresh retry rights for this daemon lifetime.
    pub fn new(instance: DaemonId) -> Self {
        Self {
            instance,
            clients: Clients::default(),
            records: HashMap::new(),
            issued: 0,
            windows: [
                Window::new(768, 12 * 1024 * 1024),
                Window::new(256, 4 * 1024 * 1024),
            ],
        }
    }

    /// Authenticate or issue a lease; unknown ids receive new rights and a new secret.
    pub fn hello(&mut self, resume: Option<Resume>, now: Instant) -> Result<HelloResult, RpcError> {
        for client in self.clients.expire(now) {
            self.forget(&client);
        }
        let (hello, evicted) = self.clients.hello(&self.instance, resume, now)?;
        if let Some(client) = evicted {
            self.forget(&client);
        }
        Ok(hello)
    }

    /// Closing a device connection leaves admitted commands running.
    pub fn disconnect(&mut self, client: &ClientId, now: Instant) {
        self.clients.disconnect(client, now);
    }

    /// Keep reads authenticated without consuming an operation slot.
    pub fn touch(&mut self, client: &ClientId, now: Instant) -> Result<(), RpcError> {
        self.clients.touch(client, now).map(|_| ())
    }

    /// Reserve both canonical bytes and recovery space before handing work to an owner.
    pub fn admit(
        &mut self,
        client: &ClientId,
        operation: ClientOperation,
        session: Option<SessionId>,
        canonical: Vec<u8>,
        class: Class,
        now: Instant,
    ) -> Result<Admission, RpcError> {
        if let Some(result) = self.existing(client, &operation, &canonical, now)? {
            return Ok(Admission::Existing(result));
        }
        let reserved = canonical
            .len()
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(4096))
            .ok_or_else(|| {
                error(
                    -32010,
                    "operation exceeds retained-result budget; nothing dispatched",
                )
            })?;
        self.reserve(class, reserved)?;
        let result = OperationResult {
            operation: operation.clone(),
            session,
            state: OperationState::Pending,
            rewound: false,
        };
        self.clients
            .records
            .get_mut(client)
            .expect("authenticated client")
            .high_water = operation.id.get();
        self.windows[class.index()].add(reserved);
        self.records.insert(
            operation,
            Record {
                canonical,
                result: result.clone(),
                class,
                reserved,
                order: self.issued,
            },
        );
        Ok(Admission::New(result))
    }

    /// Return only this client's result; a missing older counter never grants a new execution.
    pub fn read(
        &mut self,
        client: &ClientId,
        operation: &ClientOperation,
        now: Instant,
    ) -> Result<OperationResult, RpcError> {
        self.authorize(client, operation, now)?;
        self.records
            .get(operation)
            .map(|record| record.result.clone())
            .ok_or_else(expired)
    }

    /// Retain a checked owner result before any socket notification; late expired leases stay expired.
    pub fn complete(
        &mut self,
        mut result: OperationResult,
    ) -> Result<Option<OperationResult>, RpcError> {
        let Some(record) = self.records.get_mut(&result.operation) else {
            return Ok(None);
        };
        if record.result.session != result.session {
            return Err(error(
                -32603,
                "owner returned a result for a different session",
            ));
        }
        if terminal(&record.result.state) && !terminal(&result.state) {
            return Ok(Some(record.result.clone()));
        }
        result.rewound |= record.result.rewound;
        match &mut result.state {
            OperationState::Rejected { message, .. }
            | OperationState::Uncertain { message, .. } => *message = diagnostic(message),
            OperationState::Accepted {
                notice: Some(notice),
                ..
            } => *notice = diagnostic(notice),
            OperationState::Pending
            | OperationState::Queued
            | OperationState::Accepted { notice: None, .. } => {}
        }
        let bytes = record.canonical.len().saturating_add(
            serde_json::to_vec(&result)
                .map_err(|_| error(-32603, "could not encode retained result"))?
                .len(),
        );
        if bytes > record.reserved {
            return Err(error(
                -32603,
                "owner result exceeds its reserved recovery budget",
            ));
        }
        record.result = result.clone();
        Ok(Some(result))
    }

    fn reserve(&mut self, class: Class, reserved: usize) -> Result<(), RpcError> {
        if reserved > self.windows[class.index()].byte_limit {
            return Err(error(
                -32010,
                "operation exceeds retained-result budget; nothing dispatched",
            ));
        }
        while !self.windows[class.index()].fits(reserved) {
            let oldest = self
                .records
                .iter()
                .filter(|(_, record)| record.class == class && terminal(&record.result.state))
                .min_by_key(|(_, record)| record.order)
                .map(|(operation, _)| operation.clone());
            let Some(oldest) = oldest else {
                return Err(error(
                    -32010,
                    "pending operation/result capacity is full; nothing dispatched",
                ));
            };
            self.remove(&oldest);
        }
        self.issued = self.issued.checked_add(1).ok_or_else(|| {
            error(
                -32010,
                "daemon operation order exhausted; nothing dispatched",
            )
        })?;
        Ok(())
    }

    fn authorize(
        &mut self,
        client: &ClientId,
        operation: &ClientOperation,
        now: Instant,
    ) -> Result<(), RpcError> {
        if &operation.client != client {
            return Err(error(-32003, "operation belongs to another client"));
        }
        self.touch(client, now)
    }

    fn forget(&mut self, client: &ClientId) {
        let operations: Vec<_> = self
            .records
            .keys()
            .filter(|operation| &operation.client == client)
            .cloned()
            .collect();
        for operation in operations {
            self.remove(&operation);
        }
    }

    fn remove(&mut self, operation: &ClientOperation) {
        if let Some(record) = self.records.remove(operation) {
            let window = &mut self.windows[record.class.index()];
            window.count -= 1;
            window.bytes -= record.reserved;
        }
    }
}

impl Class {
    fn index(self) -> usize {
        match self {
            Self::Ordinary => 0,
            Self::Control => 1,
        }
    }
}
impl Window {
    fn new(count_limit: usize, byte_limit: usize) -> Self {
        Self {
            count: 0,
            bytes: 0,
            count_limit,
            byte_limit,
        }
    }
    fn fits(&self, bytes: usize) -> bool {
        self.count < self.count_limit && bytes <= self.byte_limit.saturating_sub(self.bytes)
    }
    fn add(&mut self, bytes: usize) {
        self.count += 1;
        self.bytes += bytes;
    }
}

fn terminal(state: &OperationState) -> bool {
    !matches!(state, OperationState::Pending | OperationState::Queued)
}
fn expired() -> RpcError {
    error(-32006, "operation outcome expired or was never admitted; do not resend uncertain work automatically")
}
fn error(code: i32, message: impl Into<String>) -> RpcError {
    RpcError {
        code,
        message: message.into(),
    }
}
fn diagnostic(message: &str) -> String {
    let mut output = String::new();
    for character in message.chars() {
        let character = if character.is_control() && !matches!(character, '\n' | '\t') {
            '\u{fffd}'
        } else {
            character
        };
        if output.len() + character.len_utf8() > 1000 {
            output.push_str(" [details truncated]");
            break;
        }
        output.push(character);
    }
    output
}

#[cfg(test)]
mod tests;
