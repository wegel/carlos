//! Save only session addresses and launch choices, without persisting transcripts or native jobs.

use std::fmt;
use std::path::Path;
use std::time::Duration;

#[path = "registry_rows.rs"]
mod rows;
#[path = "registry_schema.rs"]
mod schema;

use carlos_proto::ids::SessionId;
use carlos_proto::session::{Availability, Backend, Phase, SessionInfo};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// The catalogue preserves a logical address even when its native backend is dormant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedSession {
    pub id: SessionId,
    pub name: Option<String>,
    pub backend: Backend,
    pub cwd: String,
    pub native_thread: Option<String>,
    pub ralph_was_on: bool,
    #[serde(default)]
    pub stopped: bool,
}

/// One disk worker owns this connection; owners and the manager never borrow it through a lock.
pub struct Registry {
    connection: Connection,
}

/// Refuse invalid or newer catalogues without guessing a different native session.
#[derive(Debug)]
pub enum RegistryError {
    Sql(rusqlite::Error),
    Io(std::io::Error),
    Invalid(String),
}

impl Registry {
    /// Open a private catalogue before serving clients; callers run disk work on its worker.
    pub fn open(path: &Path) -> Result<Self, RegistryError> {
        if let Ok(metadata) = path.symlink_metadata() {
            if !metadata.file_type().is_file() {
                return Err(RegistryError::Invalid(
                    "catalogue path is not a regular file".into(),
                ));
            }
        }
        let mut connection = Connection::open(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(RegistryError::Io)?;
        }
        connection.busy_timeout(Duration::from_secs(5))?;
        schema::prepare(&mut connection)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        Ok(Self { connection })
    }

    /// Load descriptions only; this call never opens a native backend.
    pub fn load(&self) -> Result<Vec<SavedSession>, RegistryError> {
        rows::load(&self.connection)
    }

    /// Replace one description atomically after its durable fields actually change.
    pub fn save(&self, session: &SavedSession) -> Result<(), RegistryError> {
        session.validate()?;
        let inserted = self.connection.execute("INSERT INTO sessions(id,name,backend,cwd,native_thread,ralph,stopped)
            SELECT ?1,?2,?3,?4,?5,?6,?7 WHERE EXISTS(SELECT 1 FROM sessions WHERE id=?1) OR (SELECT count(*) FROM sessions) < ?8
            ON CONFLICT(id) DO UPDATE SET name=excluded.name,backend=excluded.backend,cwd=excluded.cwd,native_thread=excluded.native_thread,ralph=excluded.ralph,stopped=excluded.stopped",
            params![session.id.as_str(), session.name, serde_json::to_string(&session.backend).expect("backend enum"), session.cwd, session.native_thread, session.ralph_was_on, session.stopped, crate::limits::SESSIONS as i64])?;
        if inserted == 0 {
            return Err(RegistryError::Invalid(
                "saved-session capacity is full".into(),
            ));
        }
        Ok(())
    }
}

impl SavedSession {
    /// Show saved work without claiming that a dormant automatic loop is running.
    pub fn dormant_info(&self) -> SessionInfo {
        SessionInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            backend: self.backend,
            cwd: Some(self.cwd.clone()),
            host: "local".into(),
            model: None,
            phase: Phase::Idle,
            availability: if self.stopped {
                Availability::Closed
            } else {
                Availability::Dormant
            },
            current_turn: None,
            ralph: false,
            turn_started_at: None,
            context: None,
            quota: None,
            status: None,
        }
    }

    /// Preserve the actual native address separately from the public session description.
    pub fn from_info(info: &SessionInfo, native_thread: String) -> Result<Self, RegistryError> {
        let saved = Self {
            id: info.id.clone(),
            name: info.name.clone(),
            backend: info.backend,
            cwd: info.cwd.clone().ok_or_else(|| {
                RegistryError::Invalid("live session has no working directory".into())
            })?,
            native_thread: Some(native_thread),
            ralph_was_on: info.ralph,
            stopped: false,
        };
        saved.validate()?;
        Ok(saved)
    }

    fn validate(&self) -> Result<(), RegistryError> {
        if self.stopped && self.ralph_was_on {
            return Err(RegistryError::Invalid(
                "stopped sessions must keep Ralph off".into(),
            ));
        }
        if !Path::new(&self.cwd).is_absolute()
            || self.cwd.len() > 16384
            || self.name.as_ref().is_some_and(|name| name.len() > 4096)
            || self
                .native_thread
                .as_ref()
                .is_some_and(|id| id.is_empty() || id.len() > 4096)
        {
            return Err(RegistryError::Invalid(
                "catalogue description has an invalid or oversized launch field".into(),
            ));
        }
        Ok(())
    }
}

impl From<rusqlite::Error> for RegistryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sql(error)
    }
}
impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sql(error) => error.fmt(formatter),
            Self::Io(error) => error.fmt(formatter),
            Self::Invalid(message) => formatter.write_str(message),
        }
    }
}
impl std::error::Error for RegistryError {}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
