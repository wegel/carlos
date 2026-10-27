//! Keep labels in indexed SQLite rows instead of loading a growing database.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

use serde_json::{json, Value};

use super::config::Config;
use super::{Label, Request, Source};

pub(super) struct Store(Connection);

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_millis(250))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA cache_size=-2048;
             CREATE TABLE IF NOT EXISTS summaries (
                 command_hash BLOB PRIMARY KEY, label TEXT NOT NULL,
                 provider TEXT NOT NULL, model TEXT NOT NULL,
                 prompt_version INTEGER NOT NULL, created_at INTEGER NOT NULL
             ) WITHOUT ROWID;
             CREATE TABLE IF NOT EXISTS source_labels (
                 label_hash BLOB PRIMARY KEY, command_hash BLOB NOT NULL,
                 label TEXT NOT NULL, sources TEXT NOT NULL, provider TEXT NOT NULL,
                 model TEXT NOT NULL, prompt_version INTEGER NOT NULL, created_at INTEGER NOT NULL
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS source_labels_command ON source_labels(command_hash, created_at DESC);
             CREATE TABLE IF NOT EXISTS source_links (
                 entry TEXT PRIMARY KEY, label_hash BLOB NOT NULL
             ) WITHOUT ROWID;"
        )?;
        Ok(Self(connection))
    }

    pub fn lookup(&self, request: &Request) -> Result<Option<Label>> {
        let saved = self.0.query_row(
            "SELECT s.label,s.sources FROM source_links l JOIN source_labels s USING(label_hash)
             WHERE l.entry=?1 AND s.command_hash=?2", params![request.entry, request.key.as_slice()],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).optional()?;
        if let Some((text, sources)) = saved {
            return Ok(Some(Label {
                text,
                sources: decode_sources(&sources)?,
            }));
        }
        if !request.file_based {
            return Ok(self
                .0
                .query_row(
                    "SELECT label FROM summaries WHERE command_hash=?1",
                    [request.key.as_slice()],
                    |row| {
                        Ok(Label {
                            text: row.get(0)?,
                            sources: Vec::new(),
                        })
                    },
                )
                .optional()?);
        }
        if !request.generate {
            return Ok(None);
        }
        self.lookup_sources(request)
    }

    fn lookup_sources(&self, request: &Request) -> Result<Option<Label>> {
        let mut statement = self.0.prepare(
            "SELECT label_hash,label,sources FROM source_labels WHERE command_hash=?1 AND sources!='[]'
             ORDER BY created_at DESC LIMIT 16")?;
        let rows = statement.query_map([request.key.as_slice()], |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (hash, text, raw) = row?;
            let sources = decode_sources(&raw)?;
            if sources.iter().all(|s| source_matches(&request.cwd, s)) {
                self.0.execute(
                    "INSERT OR REPLACE INTO source_links VALUES (?1,?2)",
                    params![request.entry, hash],
                )?;
                return Ok(Some(Label { text, sources }));
            }
        }
        Ok(None)
    }

    pub fn save(&self, request: &Request, label: &Label, config: &Config) -> Result<()> {
        let provider = if config.codex { "codex" } else { "command" };
        if request.file_based || !label.sources.is_empty() {
            let sources = Value::Array(
                label
                    .sources
                    .iter()
                    .map(|s| {
                        json!({"path":s.path,
                "hash":blake3::Hash::from_bytes(s.hash).to_hex().to_string()})
                    })
                    .collect(),
            )
            .to_string();
            let hash = blake3::hash(format!("{:?}{sources}{}", request.key, label.text).as_bytes());
            let transaction = self.0.unchecked_transaction()?;
            transaction.execute(
                "INSERT OR IGNORE INTO source_labels VALUES (?1,?2,?3,?4,?5,?6,2,unixepoch())",
                params![
                    hash.as_bytes().as_slice(),
                    request.key.as_slice(),
                    label.text,
                    sources,
                    provider,
                    config.model
                ],
            )?;
            transaction.execute(
                "INSERT OR REPLACE INTO source_links VALUES (?1,?2)",
                params![request.entry, hash.as_bytes().as_slice()],
            )?;
            transaction.commit()?;
        } else {
            self.0.execute(
                "INSERT OR IGNORE INTO summaries VALUES (?1,?2,?3,?4,2,unixepoch())",
                params![request.key.as_slice(), label.text, provider, config.model],
            )?;
        }
        Ok(())
    }
}

fn decode_sources(raw: &str) -> Result<Vec<Source>> {
    let rows: Vec<Value> = serde_json::from_str(raw)?;
    rows.into_iter()
        .map(|value| {
            let hash =
                blake3::Hash::from_hex(value["hash"].as_str().context("Invalid source hash")?)?;
            Ok(Source {
                path: PathBuf::from(value["path"].as_str().context("Invalid source path")?),
                hash: *hash.as_bytes(),
            })
        })
        .collect()
}

fn source_matches(cwd: &Path, source: &Source) -> bool {
    let Ok(root) = cwd.canonicalize() else {
        return false;
    };
    let Ok(path) = root.join(&source.path).canonicalize() else {
        return false;
    };
    if !path.starts_with(root) {
        return false;
    }
    let Ok(file) = super::reader::open_source(&path) else {
        return false;
    };
    if !file
        .metadata()
        .is_ok_and(|m| m.is_file() && m.len() <= 65_536)
    {
        return false;
    }
    let mut bytes = Vec::new();
    file.take(65_537).read_to_end(&mut bytes).is_ok()
        && bytes.len() <= 65_536
        && *blake3::hash(&bytes).as_bytes() == source.hash
}
