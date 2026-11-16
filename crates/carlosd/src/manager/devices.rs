//! Admit local device writes, wait for durable results and revoke private access immediately.

use std::collections::{HashMap, HashSet};

use carlos_proto::commands::{Command, RequestId, RpcError};
use carlos_proto::devices::{Devices as DeviceList, MAX_DEVICES};
use carlos_proto::ids::{ClientId, DeviceId};
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState};

use super::Manager;
use crate::command_info::describe;
use crate::registry::{RegistryError, SavedDevice};
use crate::retry::{Admission, ClientScope};
use crate::transport::wire::error;

#[derive(Clone)]
pub(super) enum DeviceChange {
    Add(SavedDevice),
    Revoke(DeviceId),
}

impl DeviceChange {
    fn id(&self) -> &DeviceId {
        match self {
            Self::Add(device) => &device.info.id,
            Self::Revoke(id) => id,
        }
    }
}

pub(super) struct DeviceWrite {
    pub result: OperationResult,
    pub change: DeviceChange,
}

struct Pending {
    result: OperationResult,
    new_slot: bool,
}

pub(super) struct Devices {
    records: HashMap<DeviceId, SavedDevice>,
    pending: HashMap<DeviceId, Pending>,
    blocked: HashSet<DeviceId>,
}

impl Devices {
    pub(super) fn new(records: Vec<SavedDevice>) -> Self {
        Self {
            records: records
                .into_iter()
                .map(|row| (row.info.id.clone(), row))
                .collect(),
            pending: HashMap::new(),
            blocked: HashSet::new(),
        }
    }

    fn check(&self, change: &DeviceChange) -> Result<(), RpcError> {
        if self.pending.contains_key(change.id()) {
            return Err(error(
                -32010,
                "another write for that device is still pending; nothing changed",
            ));
        }
        if let DeviceChange::Add(device) = change {
            if let Some(old) = self.records.get(change.id()) {
                if old != device {
                    return Err(error(
                        -32005,
                        "device id already has different metadata or credentials",
                    ));
                }
            } else if self.records.len()
                + self
                    .pending
                    .values()
                    .filter(|pending| pending.new_slot)
                    .count()
                >= MAX_DEVICES
            {
                return Err(error(
                    -32010,
                    "device capacity is full; revoke one before adding another",
                ));
            }
        }
        Ok(())
    }
}

impl Manager {
    pub(super) fn authenticate_device(
        &self,
        token: &carlos_proto::devices::AccessToken,
    ) -> Result<DeviceId, RpcError> {
        let denied = || error(-32003, "invalid or revoked device credential");
        let (id, encoded) = token.as_str().split_once('.').ok_or_else(denied)?;
        let id: DeviceId = id.parse().map_err(|_| denied())?;
        let record = self
            .devices
            .records
            .get(&id)
            .filter(|_| !self.devices.blocked.contains(&id))
            .ok_or_else(denied)?;
        let mut secret = [0; 32];
        for (index, byte) in secret.iter_mut().enumerate() {
            *byte =
                u8::from_str_radix(&encoded[index * 2..index * 2 + 2], 16).map_err(|_| denied())?;
        }
        if blake3::hash(&secret) != record.verifier {
            return Err(denied());
        }
        Ok(id)
    }

    pub(super) fn device_command(
        &mut self,
        peer: u64,
        id: RequestId,
        command: Command,
        client: ClientId,
    ) {
        if !matches!(self.peers[&peer].scope.as_ref(), Some(ClientScope::Local)) {
            self.fail(
                peer,
                id,
                error(
                    -32003,
                    "device controls require authenticated Unix peer credentials",
                ),
            );
            return;
        }
        if matches!(command, Command::DeviceList {}) {
            let mut devices: Vec<_> = self
                .devices
                .records
                .values()
                .filter(|row| !self.devices.blocked.contains(&row.info.id))
                .map(|row| row.info.clone())
                .collect();
            devices.sort_by(|left, right| left.id.cmp(&right.id));
            self.reply(peer, id, DeviceList { devices });
            return;
        }
        self.device_mutation(peer, id, command, client);
    }

    fn device_mutation(&mut self, peer: u64, id: RequestId, command: Command, client: ClientId) {
        let canonical = serde_json::to_vec(&command).expect("typed device command");
        let class = describe(&command)
            .retry
            .expect("device mutation retry class");
        let (operation, change) = match device_change(command) {
            Ok(parts) => parts,
            Err(reason) => {
                self.fail(peer, id, reason);
                return;
            }
        };
        let result = match self.retry.admit(
            &client,
            operation,
            None,
            canonical,
            class,
            std::time::Instant::now(),
        ) {
            Ok(Admission::Existing(result)) => {
                self.reply(peer, id, result);
                return;
            }
            Ok(Admission::New(result)) => result,
            Err(reason) => {
                self.fail(peer, id, reason);
                return;
            }
        };
        self.queue_device(peer, id, result, change);
    }

    fn queue_device(
        &mut self,
        peer: u64,
        id: RequestId,
        result: OperationResult,
        change: DeviceChange,
    ) {
        if let Err(reason) = self.devices.check(&change) {
            self.reject_device(peer, id, result, reason.message);
            return;
        }
        let write = DeviceWrite {
            result: result.clone(),
            change: change.clone(),
        };
        if !self.store.save_device(write) {
            self.reject_device(
                peer,
                id,
                result,
                "catalogue write queue is full or unavailable; nothing changed".into(),
            );
            return;
        }
        let device = change.id().clone();
        let new_slot =
            matches!(change, DeviceChange::Add(_)) && !self.devices.records.contains_key(&device);
        self.devices.pending.insert(
            device.clone(),
            Pending {
                result: result.clone(),
                new_slot,
            },
        );
        if matches!(change, DeviceChange::Revoke(_)) {
            self.revoke_device_access(&device);
        }
        self.reply(peer, id, result);
    }

    fn reject_device(
        &mut self,
        peer: u64,
        id: RequestId,
        mut result: OperationResult,
        message: String,
    ) {
        result.state = OperationState::Rejected {
            message,
            recovery: None,
        };
        self.result(result.clone());
        self.reply(peer, id, result);
    }

    fn revoke_device_access(&mut self, id: &DeviceId) {
        if self.devices.records.contains_key(id) {
            self.devices.blocked.insert(id.clone());
        }
        self.retry.revoke_device(id);
        for peer in self.peers.values() {
            if matches!(peer.scope.as_ref(), Some(ClientScope::Device(device)) if device == id) {
                peer.socket.close();
            }
        }
    }

    pub(super) fn finish_device_write(
        &mut self,
        mut write: DeviceWrite,
        result: Result<(), RegistryError>,
    ) {
        let id = write.change.id().clone();
        self.devices.pending.remove(&id);
        write.result.state = match result {
            Ok(()) => {
                let action = match write.change {
                    DeviceChange::Add(device) => {
                        self.devices.records.insert(id.clone(), device);
                        "provisioned"
                    }
                    DeviceChange::Revoke(_) => {
                        self.devices.records.remove(&id);
                        "revoked"
                    }
                };
                self.devices.blocked.remove(&id);
                OperationState::Accepted { entry: None, notice: Some(format!("Device {id} {action}")) }
            }
            Err(reason) => OperationState::Uncertain {
                message: format!("device write was not confirmed: {reason}; blocked devices stay blocked until daemon restart"),
                recovery: None,
            },
        };
        self.result(write.result);
    }

    pub(super) fn device_store_failed(&mut self) {
        let results: Vec<_> = self
            .devices
            .pending
            .drain()
            .map(|(_, pending)| pending.result)
            .collect();
        for mut result in results {
            result.state = OperationState::Uncertain {
                message: "device write was not confirmed because the catalogue worker stopped; blocked devices stay blocked until daemon restart".into(),
                recovery: None,
            };
            self.result(result);
        }
    }
}

fn device_change(command: Command) -> Result<(ClientOperation, DeviceChange), RpcError> {
    match command {
        Command::DeviceAdd(request) => {
            let device = SavedDevice::from_add(&request)
                .map_err(|reason| error(-32602, reason.to_string()))?;
            Ok((request.operation, DeviceChange::Add(device)))
        }
        Command::DeviceRevoke(request) => {
            Ok((request.operation, DeviceChange::Revoke(request.device)))
        }
        _ => unreachable!("typed device mutation"),
    }
}
