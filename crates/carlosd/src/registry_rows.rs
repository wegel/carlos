//! Validate bounded saved session descriptions before accepting a catalogue or its migration.

use rusqlite::Connection;

use super::{RegistryError, SavedSession};

pub(super) fn load(connection: &Connection) -> Result<Vec<SavedSession>, RegistryError> {
    let mut statement = connection.prepare(
        "SELECT id,name,backend,cwd,native_thread,ralph,stopped,claude_fresh FROM sessions ORDER BY id LIMIT ?1",
    )?;
    let rows = statement.query_map([(crate::limits::SESSIONS + 1) as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, bool>(5)?,
            row.get::<_, bool>(6)?,
            row.get::<_, bool>(7)?,
        ))
    })?;
    let mut saved = Vec::new();
    for row in rows {
        let (id, name, backend, cwd, native_thread, ralph_was_on, stopped, claude_fresh) = row?;
        let session = SavedSession {
            id: id.parse().map_err(|_| {
                RegistryError::Invalid("catalogue contains a malformed logical id".into())
            })?,
            name,
            backend: serde_json::from_str(&backend).map_err(|_| {
                RegistryError::Invalid("catalogue contains an unsupported backend".into())
            })?,
            cwd,
            native_thread,
            ralph_was_on,
            stopped,
            claude_fresh,
        };
        session.validate()?;
        saved.push(session);
    }
    if saved.len() > crate::limits::SESSIONS {
        return Err(RegistryError::Invalid(
            "catalogue exceeds 256 saved sessions".into(),
        ));
    }
    Ok(saved)
}
