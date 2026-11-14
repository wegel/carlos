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
        .pragma_update(None, "user_version", 2)
        .expect("future version");
    assert!(Registry::open(&path).is_err());
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("version unchanged");
    assert_eq!(version, 2);
}
