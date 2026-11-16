//! Persist only bounded device metadata and BLAKE3 verifiers on the existing catalogue connection.

use blake3::Hash;
use carlos_proto::devices::{DeviceAdd, DeviceInfo, MAX_DEVICES};
use carlos_proto::ids::DeviceId;
use rusqlite::{params, Connection};

use super::{Registry, RegistryError};

/// A durable device record contains the secret's verifier, never the bearer secret itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedDevice {
    pub info: DeviceInfo,
    pub verifier: Hash,
}

impl SavedDevice {
    /// Check the operator's named payload before reserving a catalogue write.
    pub fn from_add(request: &DeviceAdd) -> Result<Self, RegistryError> {
        request
            .validate()
            .map_err(|error| RegistryError::Invalid(error.message))?;
        Ok(Self {
            info: request.device.clone(),
            verifier: parse_verifier(&request.verifier)?,
        })
    }
}

impl Registry {
    /// Load device verifiers without issuing credentials or opening a native session.
    pub fn load_devices(&self) -> Result<Vec<SavedDevice>, RegistryError> {
        load(&self.connection)
    }

    /// Admit a new credential or its exact duplicate without replacing another credential or name.
    pub fn save_device(&self, device: &SavedDevice) -> Result<(), RegistryError> {
        device
            .info
            .validate()
            .map_err(|error| RegistryError::Invalid(error.message))?;
        let verifier = device.verifier.to_hex();
        let inserted = self.connection.execute(
            "INSERT INTO devices(id,name,verifier)
             SELECT ?1,?2,?3 WHERE EXISTS(SELECT 1 FROM devices WHERE id=?1)
                 OR (SELECT count(*) FROM devices) < ?4
             ON CONFLICT(id) DO UPDATE SET name=excluded.name
                 WHERE devices.name=excluded.name AND devices.verifier=excluded.verifier",
            params![
                device.info.id.as_str(),
                device.info.name,
                verifier.as_str(),
                MAX_DEVICES as i64
            ],
        )?;
        if inserted == 0 {
            return Err(RegistryError::Invalid(
                "device capacity is full or that id already has different metadata or credentials"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Remove one device durably; repeating an already-completed revocation remains harmless.
    pub fn revoke_device(&self, id: &DeviceId) -> Result<(), RegistryError> {
        self.connection
            .execute("DELETE FROM devices WHERE id=?1", [id.as_str()])?;
        Ok(())
    }
}

pub(super) fn load(connection: &Connection) -> Result<Vec<SavedDevice>, RegistryError> {
    // Slice bytes so SQLite cannot hide embedded NULs behind its text-length rules.
    let mut statement = connection.prepare(
        "SELECT CAST(substr(CAST(id AS BLOB),1,33) AS TEXT),
                CAST(substr(CAST(name AS BLOB),1,257) AS TEXT),
                CAST(substr(CAST(verifier AS BLOB),1,65) AS TEXT),
                typeof(id)='text' AND typeof(name)='text' AND typeof(verifier)='text'
         FROM devices ORDER BY id LIMIT ?1",
    )?;
    let rows = statement.query_map([(MAX_DEVICES + 1) as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, bool>(3)?,
        ))
    })?;
    let mut devices = Vec::new();
    for row in rows {
        if devices.len() == MAX_DEVICES {
            return Err(RegistryError::Invalid("catalogue exceeds32 devices".into()));
        }
        let (id, name, verifier, valid_types) = row?;
        if !valid_types {
            return Err(RegistryError::Invalid(
                "device metadata must use text columns".into(),
            ));
        }
        let info = DeviceInfo {
            id: id.parse().map_err(|_| {
                RegistryError::Invalid("catalogue contains a malformed device id".into())
            })?,
            name,
        };
        info.validate()
            .map_err(|error| RegistryError::Invalid(error.message))?;
        devices.push(SavedDevice {
            info,
            verifier: parse_verifier(&verifier)?,
        });
    }
    Ok(devices)
}

fn parse_verifier(value: &str) -> Result<Hash, RegistryError> {
    let parsed = Hash::from_hex(value).map_err(|_| {
        RegistryError::Invalid("catalogue contains a malformed device verifier".into())
    })?;
    if parsed.to_hex().as_str() != value {
        return Err(RegistryError::Invalid(
            "device verifier is not canonical lowercase hexadecimal".into(),
        ));
    }
    Ok(parsed)
}

#[cfg(test)]
#[path = "registry_device_tests.rs"]
mod tests;
