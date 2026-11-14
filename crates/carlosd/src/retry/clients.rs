//! Retain a bounded set of client credentials without granting an expired id its old rights.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use carlos_proto::commands::{HelloResult, RpcError};
use carlos_proto::ids::{ClientId, DaemonId, OperationId, ResumeToken};
use uuid::Uuid;

use super::{error, Resume};

pub(super) struct Client {
    secret: ResumeToken,
    pub high_water: u64,
    active: usize,
    seen: Instant,
}

pub(super) struct Clients {
    pub records: HashMap<ClientId, Client>,
    pub capacity: usize,
    pub lease: Duration,
}

impl Default for Clients {
    fn default() -> Self {
        Self {
            records: HashMap::new(),
            capacity: 128,
            lease: Duration::from_secs(24 * 60 * 60),
        }
    }
}

impl Clients {
    pub fn expire(&mut self, now: Instant) -> Vec<ClientId> {
        let expired: Vec<_> = self
            .records
            .iter()
            .filter(|(_, client)| {
                client.active == 0 && now.saturating_duration_since(client.seen) >= self.lease
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in &expired {
            self.records.remove(id);
        }
        expired
    }

    pub fn hello(
        &mut self,
        instance: &DaemonId,
        resume: Option<Resume>,
        now: Instant,
    ) -> Result<(HelloResult, Option<ClientId>), RpcError> {
        if let Some(resume) = resume.filter(|resume| &resume.instance == instance) {
            if let Some(client) = self.records.get_mut(&resume.client) {
                if !equal_secret(&client.secret, &resume.secret) {
                    return Err(error(-32003, "invalid client resume credential"));
                }
                client.active += 1;
                client.seen = now;
                return Ok((greeting(instance, resume.client, client, true), None));
            }
        }
        let id = Uuid::new_v4()
            .simple()
            .to_string()
            .parse()
            .expect("UUID client id");
        let client = Client {
            secret: secret()?,
            high_water: 0,
            active: 1,
            seen: now,
        };
        let evicted = if self.records.len() >= self.capacity {
            let id = self
                .records
                .iter()
                .filter(|(_, client)| client.active == 0)
                .min_by_key(|(_, client)| client.seen)
                .map(|(id, _)| id.clone())
                .ok_or_else(|| error(-32010, "client identity capacity is full"))?;
            self.records.remove(&id);
            Some(id)
        } else {
            None
        };
        let result = greeting(instance, id, &client, false);
        self.records.insert(result.client.clone(), client);
        Ok((result, evicted))
    }

    pub fn touch(&mut self, id: &ClientId, now: Instant) -> Result<&mut Client, RpcError> {
        let client = self.records.get_mut(id).ok_or_else(|| {
            error(
                -32004,
                "client identity expired; greet again before recovering uncertain work",
            )
        })?;
        client.seen = now;
        Ok(client)
    }

    pub fn disconnect(&mut self, id: &ClientId, now: Instant) {
        if let Some(client) = self.records.get_mut(id) {
            client.active = client.active.saturating_sub(1);
            client.seen = now;
        }
    }
}

fn greeting(instance: &DaemonId, id: ClientId, client: &Client, resumed: bool) -> HelloResult {
    HelloResult {
        protocol: carlos_proto::PROTOCOL_VERSION,
        host: "local".into(),
        daemon_version: env!("CARGO_PKG_VERSION").into(),
        instance: instance.clone(),
        client: id,
        resumed,
        next_operation: client.high_water.checked_add(1).and_then(OperationId::new),
        resume_token: client.secret.clone(),
    }
}

fn secret() -> Result<ResumeToken, RpcError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| {
        error(
            -32603,
            "OS randomness unavailable; no client identity issued",
        )
    })?;
    let encoded: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(encoded.parse().expect("32-byte private token"))
}

fn equal_secret(left: &ResumeToken, right: &ResumeToken) -> bool {
    left.as_str()
        .bytes()
        .zip(right.as_str().bytes())
        .fold(0, |difference, (left, right)| difference | (left ^ right))
        == 0
}
