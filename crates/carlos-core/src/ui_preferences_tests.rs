//! Keep startup precedence and field edits correct across independent writers and file aliases.

use std::fs;
use std::sync::{mpsc, Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tempfile::tempdir;

use super::{load_summary_enabled_from, save_fields_to};

#[test]
fn summary_flag_keeps_exact_environment_saved_and_default_rules() {
    let directory = tempdir().expect("preferences root");
    let path = directory.path().join("ui-settings.json");
    for raw in ["not JSON", "{}", "null", r#"{"ai_tool_labels":"false"}"#] {
        fs::write(&path, raw).expect("saved fixture");
        assert!(load_summary_enabled_from(Some(&path), None));
    }
    fs::write(&path, r#"{"ai_tool_labels":false}"#).expect("saved false");
    assert!(!load_summary_enabled_from(Some(&path), None));
    for value in ["on", "true", "1"] {
        assert!(load_summary_enabled_from(Some(&path), Some(value)));
    }
    for value in ["off", "false", "0", "ON", " true ", ""] {
        assert!(!load_summary_enabled_from(None, Some(value)));
    }
    assert!(load_summary_enabled_from(None, None));
    assert!(load_summary_enabled_from(
        Some(&directory.path().join("missing")),
        None
    ));
}

#[test]
fn appearance_and_label_writers_keep_each_others_fields_and_remove_only_legacy_layout() {
    let directory = tempdir().expect("preferences root");
    let path = directory.path().join("ui-settings.json");
    fs::write(
        &path,
        json!({"layout":"flow","future":{"keep":true}}).to_string(),
    )
    .expect("initial fields");
    let start = Arc::new(Barrier::new(3));
    let workers = [
        ("ai_tool_labels", json!(false)),
        ("themes", json!({"phone":"flow","desktop":"gutter"})),
    ]
    .into_iter()
    .map(|(key, value)| {
        let path = path.clone();
        let start = start.clone();
        thread::spawn(move || {
            start.wait();
            for _ in 0..50 {
                save_fields_to(&path, &[(key, value.clone())]).expect("serialized edit");
            }
        })
    })
    .collect::<Vec<_>>();
    start.wait();
    for worker in workers {
        worker.join().expect("writer");
    }
    let saved: Value =
        serde_json::from_slice(&fs::read(&path).expect("saved fields")).expect("JSON");
    assert_eq!(
        saved,
        json!({"ai_tool_labels":false,"themes":{"phone":"flow","desktop":"gutter"},"future":{"keep":true}})
    );
}

#[test]
fn a_waiting_writer_rereads_only_after_the_other_writer_publishes() {
    let directory = tempdir().expect("preferences root");
    let path = directory.path().join("ui-settings.json");
    fs::write(&path, "{}").expect("initial fields");
    let guard = super::lock::acquire(&path).expect("first writer lock");
    let worker_path = path.clone();
    let (started, waiting) = mpsc::channel();
    let worker = thread::spawn(move || {
        started.send(()).expect("writer started");
        save_fields_to(&worker_path, &[("ai_tool_labels", json!(false))]).expect("waiting edit");
    });
    waiting
        .recv_timeout(Duration::from_secs(1))
        .expect("waiting writer");
    fs::write(&path, r#"{"tool_output":"full"}"#).expect("first writer publishes while locked");
    drop(guard);
    worker.join().expect("second writer");
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&path).expect("saved JSON")).expect("JSON"),
        json!({"tool_output":"full","ai_tool_labels":false})
    );
}

#[cfg(unix)]
#[test]
fn file_aliases_share_the_lock_and_keep_the_original_symlink() {
    let directory = tempdir().expect("preferences root");
    let target = directory.path().join("target.json");
    let alias = directory.path().join("alias.json");
    fs::write(&target, r#"{"tool_output":"compact"}"#).expect("target");
    std::os::unix::fs::symlink(&target, &alias).expect("settings symlink");
    save_fields_to(&alias, &[("ai_tool_labels", json!(true))]).expect("edit through link");
    assert!(fs::symlink_metadata(&alias)
        .expect("link metadata")
        .file_type()
        .is_symlink());
    assert_eq!(
        super::lock::destination(&alias).expect("resolved alias"),
        target
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&target).expect("target bytes")).expect("JSON"),
        json!({"tool_output":"compact","ai_tool_labels":true})
    );
}

#[cfg(unix)]
#[test]
fn a_stuck_file_lock_has_a_two_second_deadline_and_never_rewrites_fields() {
    let directory = tempdir().expect("preferences root");
    let path = directory.path().join("ui-settings.json");
    fs::write(&path, "{}").expect("initial fields");
    let _guard = super::lock::acquire(&path).expect("held lock");
    let start = Instant::now();
    let error =
        save_fields_to(&path, &[("ai_tool_labels", json!(false))]).expect_err("bounded lock wait");
    assert!(error.to_string().contains("two seconds"));
    assert!(start.elapsed() >= Duration::from_secs(2));
    assert_eq!(fs::read_to_string(&path).expect("unchanged file"), "{}");
}
