//! Restore historical snapshots while validating files for new live invocations.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use crate::summaries::config::Config;
use crate::summaries::store::Store;
use crate::summaries::{command_key, CommandLocation, Label, Request, Source};

fn config(root: &std::path::Path) -> Config {
    Config {
        executable: "unused".into(),
        arguments: Vec::new(),
        model: "test".into(),
        codex: false,
        timeout: Duration::from_secs(1),
        state: root.to_owned(),
    }
}

fn request(root: &std::path::Path, entry: &str, file_based: bool) -> Request {
    let command = "python3 process.py".to_owned();
    Request {
        key: command_key(&command),
        command,
        location: CommandLocation::at(root.to_owned()),
        generate: true,
        entry: entry.into(),
        file_based,
    }
}

#[test]
fn command_only_labels_survive_reopen_and_model_changes() {
    let root = tempfile::tempdir().expect("tempdir");
    let path = root.path().join("labels.sqlite3");
    let request = request(root.path(), "first", false);
    let label = Label {
        text: "Rewrite the report".into(),
        sources: Vec::new(),
    };
    Store::open(&path)
        .expect("open")
        .save(&request, &label, &config(root.path()))
        .expect("save");
    let mut resumed = request.clone();
    resumed.generate = false;
    resumed.location.cwd = PathBuf::from("/unavailable/directory");
    assert_eq!(
        Store::open(&path)
            .expect("reopen")
            .lookup(&resumed)
            .expect("lookup")
            .expect("label")
            .text,
        label.text
    );
}

#[test]
fn changed_files_get_new_labels_while_resume_keeps_original_snapshot() {
    let root = tempfile::tempdir().expect("tempdir");
    let script = root.path().join("process.py");
    fs::write(&script, "print(1)\n").expect("source");
    let store = Store::open(&root.path().join("labels.sqlite3")).expect("open");
    let original = request(root.path(), "thread:first", true);
    let label = Label {
        text: "Print one".into(),
        sources: vec![Source {
            path: PathBuf::from("process.py"),
            hash: *blake3::hash(b"print(1)\n").as_bytes(),
        }],
    };
    store
        .save(&original, &label, &config(root.path()))
        .expect("save");
    let same = request(root.path(), "thread:same", true);
    assert!(store.lookup(&same).expect("same input").is_some());
    fs::write(&script, "print(2)\n").expect("edit source");
    let changed = request(root.path(), "thread:changed", true);
    assert!(store.lookup(&changed).expect("changed input").is_none());
    let mut resumed = original;
    resumed.generate = false;
    fs::remove_file(script).expect("remove source");
    assert_eq!(
        store
            .lookup(&resumed)
            .expect("resume")
            .expect("saved snapshot")
            .text,
        "Print one"
    );
    assert_eq!(
        store
            .lookup(&same)
            .expect("second snapshot")
            .expect("saved link")
            .text,
        "Print one"
    );
}

#[test]
fn uncached_history_never_needs_source_files_or_generation() {
    let root = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&root.path().join("labels.sqlite3")).expect("open");
    let mut history = request(root.path(), "historical", true);
    history.generate = false;
    assert!(store.lookup(&history).expect("lookup").is_none());
}

#[test]
fn normalized_cache_resolves_root_relative_audit_paths_from_a_subdirectory_without_cross_context_reuse(
) {
    let root = tempfile::tempdir().expect("root");
    let cwd = root.path().join("scripts");
    fs::create_dir(&cwd).expect("scripts");
    fs::write(root.path().join("shared.py"), "shared").expect("source");
    let location = CommandLocation {
        cwd,
        read_root: root.path().to_owned(),
        directory: Some("scripts".into()),
    };
    let mut original = request(root.path(), "first", true);
    original.location = location;
    original.key = crate::summaries::presentation_key(
        "cd ./scripts && python3 ../shared.py",
        &original.location,
    );
    let store = Store::open(&root.path().join("labels.sqlite3")).expect("store");
    let label = Label {
        text: "Read the shared source".into(),
        sources: vec![Source {
            path: "shared.py".into(),
            hash: *blake3::hash(b"shared").as_bytes(),
        }],
    };
    store
        .save(&original, &label, &config(root.path()))
        .expect("save");
    let mut next = original.clone();
    next.entry = "new-live-item".into();
    assert_eq!(
        store
            .lookup(&next)
            .expect("root-relative cache")
            .expect("label")
            .text,
        label.text
    );
    fs::write(root.path().join("shared.py"), "changed").expect("change");
    next.entry = "changed-source".into();
    assert!(store.lookup(&next).expect("changed source").is_none());
    let outside = tempfile::tempdir().expect("outside");
    next.location.cwd = outside.path().to_owned();
    assert!(store
        .lookup(&next)
        .expect("outside cwd cannot reuse source")
        .is_none());
    original.generate = false;
    assert_eq!(
        store
            .lookup(&original)
            .expect("saved history")
            .expect("label")
            .text,
        label.text
    );
}
