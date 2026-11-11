//! Keep disk lookups responsive during a blocked model call and share duplicate work.

use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::summaries::config::Config;
use crate::summaries::store::Store;
use crate::summaries::{
    command_key, CommandLocation, Label, Request, SummaryEvent, SummaryService,
};

fn request(root: &Path, command: &str, entry: &str, generate: bool) -> Request {
    Request {
        key: command_key(command),
        command: command.into(),
        location: CommandLocation::at(root.into()),
        generate,
        entry: entry.into(),
        file_based: false,
    }
}

fn config(root: &Path) -> Config {
    Config { executable: "sh".into(), arguments: vec!["-c".into(),
        "printf x >> \"$1/calls\"; while [ ! -f \"$1/release\" ]; do sleep 0.01; done; printf 'Rewrite the result report'".into(),
        "worker".into(), root.to_string_lossy().into_owned()], model: "fixture".into(),
        codex: false, timeout: Duration::from_secs(10), state: root.into() }
}

fn wait_for(path: &Path) {
    let started = Instant::now();
    while !path.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "worker did not reach test gate"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn cached_and_uncached_history_do_not_wait_for_model_or_trigger_calls() {
    let root = tempfile::tempdir().expect("tempdir");
    let config = config(root.path());
    let cached = request(root.path(), "cached command", "history-hit", false);
    let store = Store::open(&root.path().join("tool-summaries.sqlite3")).expect("store");
    store
        .save(
            &cached,
            &Label {
                text: "Saved label".into(),
                sources: Vec::new(),
            },
            &config,
        )
        .expect("seed");
    let (tx, rx) = mpsc::channel();
    let service = SummaryService::spawn_config(tx, config);
    assert!(service.try_request(request(root.path(), "slow command", "live-1", true)));
    wait_for(&root.path().join("calls"));
    assert!(service.try_request(request(root.path(), "slow command", "live-2", true)));
    assert!(service.try_request(cached));
    assert!(service.try_request(request(
        root.path(),
        "uncached command",
        "history-miss",
        false
    )));
    let first = rx
        .recv_timeout(Duration::from_secs(1))
        .expect("cached lookup bypasses blocked model");
    assert!(
        matches!(first, SummaryEvent::Ready(_, ref entry, Some(ref label)) if entry=="history-hit" && label=="Saved label")
    );
    let second = rx
        .recv_timeout(Duration::from_secs(1))
        .expect("history miss completes");
    assert!(matches!(second, SummaryEvent::Ready(_, ref entry, None) if entry=="history-miss"));
    assert_eq!(
        fs::read_to_string(root.path().join("calls")).expect("calls"),
        "x"
    );
    fs::write(root.path().join("release"), "").expect("release worker");
    for _ in 0..2 {
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(3))
                .expect("live result"),
            SummaryEvent::Ready(_, _, Some(_))
        ));
    }
    assert_eq!(
        fs::read_to_string(root.path().join("calls")).expect("deduplicated calls"),
        "x"
    );
}

#[test]
fn disabling_cancels_the_child_and_keeps_fallback_without_saving_a_label() {
    let root = tempfile::tempdir().expect("tempdir");
    let (tx, rx) = mpsc::channel();
    let service = SummaryService::spawn_config(tx, config(root.path()));
    let request = request(root.path(), "blocked command", "live", true);
    assert!(service.try_request(request.clone()));
    wait_for(&root.path().join("calls"));
    service.set_enabled(false);
    assert!(rx.recv_timeout(Duration::from_millis(150)).is_err());
    let store = Store::open(&root.path().join("tool-summaries.sqlite3")).expect("store");
    assert!(store.lookup(&request).expect("lookup").is_none());
}

#[test]
fn pending_file_jobs_with_the_same_entry_but_different_context_keys_never_share_a_result() {
    let root = tempfile::tempdir().expect("root");
    let old = root.path().join("old");
    let new = root.path().join("new");
    fs::create_dir(&old).expect("old");
    fs::create_dir(&new).expect("new");
    let mut config = config(root.path());
    config.arguments[1] = "printf x >> \"$1/calls\"; while [ ! -f \"$1/release\" ]; do sleep .01; done; printf 'Describe script in %s' \"${PWD##*/}\"".into();
    let (tx, rx) = mpsc::channel();
    let service = SummaryService::spawn_config(tx, config);
    let make = |cwd| {
        let mut request = request(root.path(), "python3 process.py", "same-entry", true);
        request.location = CommandLocation {
            cwd,
            read_root: root.path().to_owned(),
            directory: None,
        };
        request.key = crate::summaries::presentation_key("same raw command", &request.location);
        request.file_based = true;
        request
    };
    let first = make(old);
    let second = make(new);
    assert_ne!(first.key, second.key);
    assert!(service.try_request(first.clone()));
    wait_for(&root.path().join("calls"));
    assert!(service.try_request(second.clone()));
    fs::write(root.path().join("release"), "").expect("release");
    let one = rx.recv_timeout(Duration::from_secs(3)).expect("old result");
    let two = rx.recv_timeout(Duration::from_secs(3)).expect("new result");
    assert!(
        matches!(one, SummaryEvent::Ready(key, _, Some(ref text)) if key==first.key && text.ends_with("old"))
    );
    assert!(
        matches!(two, SummaryEvent::Ready(key, _, Some(ref text)) if key==second.key && text.ends_with("new"))
    );
    assert_eq!(
        fs::read_to_string(root.path().join("calls")).expect("calls"),
        "xx"
    );
}

#[test]
fn source_generation_does_not_run_in_an_outside_or_symlinked_directory() {
    let root = tempfile::tempdir().expect("root");
    let outside = tempfile::tempdir().expect("outside");
    let (tx, rx) = mpsc::channel();
    let service = SummaryService::spawn_config(tx, config(root.path()));
    let mut input = request(root.path(), "python3 private.py", "outside", true);
    input.file_based = true;
    input.location.cwd = outside.path().to_owned();
    assert!(service.try_request(input.clone()));
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(1)).expect("declined"),
        SummaryEvent::Ready(_, _, None)
    ));
    let link = root.path().join("linked");
    std::os::unix::fs::symlink(outside.path(), &link).expect("link");
    input.location.cwd = link;
    input.entry = "symlink".into();
    assert!(service.try_request(input));
    assert!(matches!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("declined link"),
        SummaryEvent::Ready(_, _, None)
    ));
    assert!(!root.path().join("calls").exists());
}
