//! Upgrade session and device metadata atomically after validating their stored rows.

use rusqlite::{Connection, TransactionBehavior};

use super::{devices, rows, RegistryError};

pub(super) fn prepare(connection: &mut Connection) -> Result<(), RegistryError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: u32 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    match version {
        0 => {
            transaction.execute_batch(
                "CREATE TABLE sessions (
                id TEXT PRIMARY KEY, name TEXT, backend TEXT NOT NULL, cwd TEXT NOT NULL,
                native_thread TEXT, ralph INTEGER NOT NULL CHECK(ralph IN (0,1)),
                stopped INTEGER NOT NULL DEFAULT 0 CHECK(stopped IN (0,1)),
                claude_fresh INTEGER NOT NULL DEFAULT 0 CHECK(claude_fresh IN (0,1)));",
            )?;
        }
        1 => {
            transaction.execute_batch("ALTER TABLE sessions ADD COLUMN stopped INTEGER NOT NULL DEFAULT 0 CHECK(stopped IN (0,1));")?;
            add_claude_fresh(&transaction)?;
        }
        2 => add_claude_fresh(&transaction)?,
        3 | 4 => {}
        _ => {
            return Err(RegistryError::Invalid(format!(
                "catalogue version {version} is newer than this daemon; no rows changed"
            )))
        }
    }
    if version < 4 {
        transaction.execute_batch(
            "CREATE TABLE devices (
             id TEXT PRIMARY KEY NOT NULL CHECK(length(id)=32),
             name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 256),
             verifier TEXT NOT NULL CHECK(length(verifier)=64));",
        )?;
    }
    rows::load(&transaction)?;
    devices::load(&transaction)?;
    if version < 4 {
        transaction.pragma_update(None, "user_version", 4)?;
    }
    transaction.commit()?;
    Ok(())
}

fn add_claude_fresh(connection: &Connection) -> Result<(), RegistryError> {
    connection.execute_batch("ALTER TABLE sessions ADD COLUMN claude_fresh INTEGER NOT NULL DEFAULT 0 CHECK(claude_fresh IN (0,1));")?;
    Ok(())
}
