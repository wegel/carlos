//! Keep restart metadata separate from a running backend, a transcript and automatic work.

use carlos_proto::session::{Availability, Backend};

use super::{Registry, SavedSession};

fn saved() -> SavedSession {
    SavedSession {
        id: "a".repeat(32).parse().expect("logical id"),
        name: Some("saved work".into()),
        backend: Backend::Claude,
        cwd: "/work".into(),
        native_thread: Some("native-thread".into()),
        ralph_was_on: true,
        stopped: false,
        claude_fresh: false,
    }
}

#[test]
fn restart_lists_the_same_logical_session_without_restarting_automatic_work() {
    let root = tempfile::tempdir().expect("private catalogue");
    let path = root.path().join("sessions.sqlite3");
    let registry = Registry::open(&path).expect("new catalogue");
    registry.save(&saved()).expect("save");
    drop(registry);
    let restored = Registry::open(&path)
        .expect("reopen")
        .load()
        .expect("load descriptions");
    assert_eq!(restored, [saved()]);
    let info = restored[0].dormant_info();
    assert_eq!(info.id, saved().id);
    assert_eq!(info.availability, Availability::Dormant);
    assert!(!info.ralph);
    let connection = rusqlite::Connection::open(&path).expect("inspect tables");
    let count: u32 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table'",
            [],
            |row| row.get(0),
        )
        .expect("table count");
    assert_eq!(count, 1, "the catalogue stores no transcript or job table");
}

#[test]
fn a_missing_native_address_stays_missing_instead_of_creating_a_different_conversation() {
    let root = tempfile::tempdir().expect("private catalogue");
    let registry = Registry::open(&root.path().join("db")).expect("open");
    let mut session = saved();
    session.native_thread = None;
    registry
        .save(&session)
        .expect("record incomplete launch explicitly");
    assert!(registry.load().expect("saved description")[0]
        .native_thread
        .is_none());
}

#[test]
fn a_newer_catalogue_is_rejected_without_rewriting_its_version() {
    let root = tempfile::tempdir().expect("private catalogue");
    let path = root.path().join("db");
    let connection = rusqlite::Connection::open(&path).expect("future catalogue");
    connection
        .pragma_update(None, "user_version", 4)
        .expect("future version");
    assert!(Registry::open(&path).is_err());
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("version unchanged");
    assert_eq!(version, 4);
}

#[test]
fn stopped_catalogue_retains_the_native_address_and_never_claims_ready_or_ralph() {
    let root = tempfile::tempdir().expect("private catalogue");
    let path = root.path().join("db");
    let mut stopped = saved();
    stopped.stopped = true;
    stopped.ralph_was_on = false;
    Registry::open(&path)
        .expect("open")
        .save(&stopped)
        .expect("save stopped work");
    let restored = Registry::open(&path)
        .expect("restart")
        .load()
        .expect("load");
    assert_eq!(restored, [stopped]);
    assert_eq!(
        restored[0].dormant_info().availability,
        Availability::Closed
    );
    assert!(!restored[0].dormant_info().ralph);
    let mut invalid = restored[0].clone();
    invalid.ralph_was_on = true;
    assert!(Registry::open(&path).expect("open").save(&invalid).is_err());
}

#[test]
fn version_one_upgrade_preserves_native_metadata_and_is_repeatable() {
    let root = tempfile::tempdir().expect("private catalogue");
    let path = root.path().join("db");
    let connection = old_catalogue(&path, saved().id.as_str());
    for _ in 0..2 {
        let registry = Registry::open(&path).expect("migrate or reopen");
        assert_eq!(registry.load().expect("original descriptions"), [saved()]);
    }
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("version");
    assert_eq!(version, 3);
    let stopped: bool = connection
        .query_row("SELECT stopped FROM sessions", [], |row| row.get(0))
        .expect("new flag");
    assert!(!stopped);
}

#[test]
fn invalid_old_rows_roll_back_the_column_and_version_together() {
    let root = tempfile::tempdir().expect("private catalogue");
    let path = root.path().join("db");
    let connection = old_catalogue(&path, "");
    assert!(Registry::open(&path).is_err());
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("version");
    assert_eq!(version, 1);
    let columns: u32 = connection
        .query_row(
            "SELECT count(*) FROM pragma_table_info('sessions') WHERE name='stopped'",
            [],
            |row| row.get(0),
        )
        .expect("old schema");
    assert_eq!(columns, 0);
    let id: String = connection
        .query_row("SELECT id FROM sessions", [], |row| row.get(0))
        .expect("original bad row");
    assert!(id.is_empty());
}

fn old_catalogue(path: &std::path::Path, id: &str) -> rusqlite::Connection {
    let connection = rusqlite::Connection::open(path).expect("version-one fixture");
    connection
        .execute_batch(
            "CREATE TABLE sessions (
        id TEXT PRIMARY KEY, name TEXT, backend TEXT NOT NULL, cwd TEXT NOT NULL,
        native_thread TEXT, ralph INTEGER NOT NULL CHECK(ralph IN (0,1)));
        PRAGMA user_version=1;",
        )
        .expect("old schema");
    let original = saved();
    connection.execute("INSERT INTO sessions(id,name,backend,cwd,native_thread,ralph) VALUES(?1,?2,?3,?4,?5,?6)", rusqlite::params![
        id, original.name, serde_json::to_string(&original.backend).expect("backend"),
        original.cwd, original.native_thread, original.ralph_was_on,
    ]).expect("old description");
    connection
}

#[test]
fn fresh_claude_target_survives_restart_and_cannot_be_assigned_to_another_backend() {
    let root = tempfile::tempdir().expect("private catalogue");
    let path = root.path().join("db");
    let mut session = saved();
    session.claude_fresh = true;
    session.stopped = true;
    session.ralph_was_on = false;
    session.native_thread = Some("11111111-1111-4111-8111-111111111111".into());
    Registry::open(&path)
        .expect("open")
        .save(&session)
        .expect("fresh target");
    assert_eq!(
        Registry::open(&path)
            .expect("restart")
            .load()
            .expect("load"),
        [session.clone()]
    );
    session.backend = Backend::Grok;
    assert!(Registry::open(&path).expect("open").save(&session).is_err());
    session.backend = Backend::Claude;
    session.native_thread = None;
    assert!(Registry::open(&path).expect("open").save(&session).is_err());
}

#[test]
fn version_two_stopped_row_keeps_its_exact_saved_target_during_upgrade() {
    let root = tempfile::tempdir().expect("private catalogue");
    let path = root.path().join("db");
    let connection = old_catalogue(&path, saved().id.as_str());
    connection.execute_batch("ALTER TABLE sessions ADD COLUMN stopped INTEGER NOT NULL DEFAULT 0; UPDATE sessions SET stopped=1,ralph=0; PRAGMA user_version=2;").expect("previous stopped schema");
    let restored = Registry::open(&path)
        .expect("upgrade")
        .load()
        .expect("saved row");
    assert!(restored[0].stopped);
    assert!(!restored[0].claude_fresh);
    assert_eq!(restored[0].native_thread, saved().native_thread);
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("new version");
    assert_eq!(version, 3);
}
