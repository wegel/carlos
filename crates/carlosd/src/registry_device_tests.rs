//! Preserve exact verifier records across restart, bounded admission and rejected catalogue rows.

use carlos_proto::devices::{DeviceInfo, MAX_DEVICES};
use carlos_proto::session::Backend;
use rusqlite::Connection;

use crate::registry::{Registry, SavedDevice, SavedSession};

fn device(index: usize) -> SavedDevice {
    SavedDevice {
        info: DeviceInfo {
            id: format!("{index:032x}").parse().expect("device id"),
            name: format!("Phone {index}"),
        },
        verifier: blake3::hash(&[b'z'; 32]),
    }
}

#[test]
fn restart_retains_only_the_verifier_and_exact_duplicate_adds_cannot_replace_it() {
    let root = tempfile::tempdir().expect("private catalogue");
    let path = root.path().join("db");
    let registry = Registry::open(&path).expect("open");
    let original = device(1);
    registry.save_device(&original).expect("add");
    registry.save_device(&original).expect("identical retry");
    let mut changed = original.clone();
    changed.verifier = blake3::hash(b"another credential");
    assert!(registry.save_device(&changed).is_err());
    changed = original.clone();
    changed.info.name = "different name".into();
    assert!(registry.save_device(&changed).is_err());
    drop(registry);
    let registry = Registry::open(&path).expect("restart");
    assert_eq!(
        registry.load_devices().expect("records").as_slice(),
        std::slice::from_ref(&original)
    );
    registry.revoke_device(&original.info.id).expect("revoke");
    registry
        .revoke_device(&original.info.id)
        .expect("repeat revoke");
    drop(registry);
    assert!(Registry::open(&path)
        .expect("restart")
        .load_devices()
        .expect("records")
        .is_empty());
    for entry in root.path().read_dir().expect("catalogue files") {
        let bytes = std::fs::read(entry.expect("entry").path()).expect("database bytes");
        assert!(!bytes.windows(32).any(|window| window == [b'z'; 32]));
    }
}

#[test]
fn full_device_capacity_preserves_existing_grants_and_revocation_frees_one_slot() {
    let root = tempfile::tempdir().expect("private catalogue");
    let registry = Registry::open(&root.path().join("db")).expect("open");
    for index in 0..MAX_DEVICES {
        registry.save_device(&device(index)).expect("bounded add");
    }
    assert!(registry.save_device(&device(MAX_DEVICES)).is_err());
    registry
        .save_device(&device(1))
        .expect("duplicate at capacity");
    registry
        .revoke_device(&device(0).info.id)
        .expect("free slot");
    registry
        .save_device(&device(MAX_DEVICES))
        .expect("replacement");
    let rows = registry.load_devices().expect("records");
    assert_eq!(rows.len(), MAX_DEVICES);
    assert!(!rows.iter().any(|row| row.info.id == device(0).info.id));
}

#[test]
fn version_three_upgrade_preserves_the_fresh_native_target_and_is_repeatable() {
    let root = tempfile::tempdir().expect("private catalogue");
    let path = root.path().join("db");
    let session = SavedSession {
        id: "a".repeat(32).parse().expect("session"),
        name: None,
        backend: Backend::Claude,
        cwd: "/work".into(),
        native_thread: Some("11111111-1111-4111-8111-111111111111".into()),
        ralph_was_on: false,
        stopped: true,
        claude_fresh: true,
    };
    Registry::open(&path)
        .expect("open")
        .save(&session)
        .expect("session");
    let connection = Connection::open(&path).expect("old fixture");
    connection
        .execute_batch("DROP TABLE devices; PRAGMA user_version=3;")
        .expect("version3");
    for _ in 0..2 {
        let registry = Registry::open(&path).expect("upgrade or restart");
        assert!(registry.load_devices().expect("devices").is_empty());
        assert_eq!(
            registry.load().expect("session").as_slice(),
            std::slice::from_ref(&session)
        );
    }
    let version: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("version");
    assert_eq!(version, 4);
}

#[test]
fn embedded_nuls_and_wrong_column_types_are_rejected_without_clipping_or_rewriting_rows() {
    use rusqlite::types::Value;

    let corruptions = [
        ("name", Value::Text("Phone\0hidden".into())),
        ("id", Value::Text(format!("{}\0hidden", device(1).info.id))),
        (
            "verifier",
            Value::Text(format!("{}\0hidden", device(1).verifier.to_hex())),
        ),
        ("name", Value::Blob(b"Phone".to_vec())),
    ];
    for (column, invalid) in corruptions {
        let root = tempfile::tempdir().expect("private catalogue");
        let path = root.path().join("db");
        Registry::open(&path)
            .expect("open")
            .save_device(&device(1))
            .expect("device");
        let connection = Connection::open(&path).expect("corrupt fixture");
        connection
            .execute(&format!("UPDATE devices SET {column}=?1"), [&invalid])
            .expect("bad row");
        assert!(Registry::open(&path).is_err(), "must reject {column}");
        let kept: Value = connection
            .query_row(&format!("SELECT {column} FROM devices"), [], |row| {
                row.get(0)
            })
            .expect("untouched row");
        assert_eq!(kept, invalid);
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("version");
        assert_eq!(version, 4);
    }
}
