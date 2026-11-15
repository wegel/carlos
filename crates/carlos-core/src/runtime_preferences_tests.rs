//! Check defaults compatibility and whole-file writes without touching user files.

use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Barrier;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tempfile::tempdir;

use crate::runtime_preferences::{load_from, path_from, publish, save_to};
use crate::session::runtime_settings::RuntimeDefaults;

#[test]
fn path_keeps_xdg_then_home_fallback_without_changing_the_environment() {
    let path = |config: Option<&str>, home: Option<&str>| {
        path_from(config.map(OsString::from), home.map(OsString::from))
    };
    assert_eq!(
        path(Some("/config"), Some("/home")),
        Some("/config/carlos/runtime-defaults.json".into())
    );
    assert_eq!(
        path(Some(""), Some("/home")),
        Some("/home/.config/carlos/runtime-defaults.json".into())
    );
    assert_eq!(
        path(None, Some("/home")),
        Some("/home/.config/carlos/runtime-defaults.json".into())
    );
    assert_eq!(
        path(None, Some("")),
        Some(".config/carlos/runtime-defaults.json".into())
    );
    assert_eq!(path(None, None), None);
    assert_eq!(path(Some(""), None), None);
}

#[test]
fn load_keeps_old_fallbacks_and_trims_each_string_independently() {
    let directory = tempdir().expect("temporary settings");
    let path = directory.path().join("runtime-defaults.json");
    assert_eq!(load_from(&path), RuntimeDefaults::default());
    for contents in [
        "not JSON",
        "null",
        "[]",
        "7",
        r#"{"model":7,"effort":false,"summary":[]}"#,
        r#"{"model":" ","effort":"","summary":null}"#,
    ] {
        fs::write(&path, contents).expect("fixture");
        assert_eq!(load_from(&path), RuntimeDefaults::default(), "{contents}");
    }
    fs::write(&path, [0xff]).expect("invalid UTF-8 fixture");
    assert_eq!(load_from(&path), RuntimeDefaults::default());
    fs::write(
        &path,
        r#"{"model":" 模型 ","effort":7,"summary":" detailed ","extra":true}"#,
    )
    .expect("mixed fixture");
    assert_eq!(
        load_from(&path),
        RuntimeDefaults {
            model: Some("模型".into()),
            effort: None,
            summary: Some("detailed".into()),
        }
    );
    assert_eq!(load_from(directory.path()), RuntimeDefaults::default());
}

#[test]
fn save_creates_parents_and_keeps_the_existing_json_schema() {
    let directory = tempdir().expect("temporary settings");
    let path = directory.path().join("nested/runtime-defaults.json");
    let defaults = choices("模型\nquoted\"choice");
    save_to(&path, &defaults).expect("save choices");
    assert_eq!(load_from(&path), defaults);
    assert_eq!(
        read_json(&path),
        json!({"model":defaults.model,"effort":defaults.effort,"summary":defaults.summary})
    );
    save_to(&path, &RuntimeDefaults::default()).expect("replace choices");
    assert_eq!(
        read_json(&path),
        json!({"model":null,"effort":null,"summary":null})
    );
    assert_eq!(
        fs::read_dir(path.parent().expect("parent"))
            .expect("directory")
            .count(),
        1
    );
}

#[test]
fn errors_keep_prior_contents_and_remove_owned_temporary_files() {
    let directory = tempdir().expect("temporary settings");
    let path = directory.path().join("runtime-defaults.json");
    fs::write(&path, b"previous file").expect("prior file");
    assert!(save_to(&path.join("child"), &choices("new")).is_err());
    assert_eq!(
        fs::read(&path).expect("prior file remains"),
        b"previous file"
    );
    let blocked = directory.path().join("directory-target");
    fs::create_dir(&blocked).expect("directory target");
    fs::write(blocked.join("marker"), b"keep").expect("prior directory contents");
    assert_eq!(
        save_to(&blocked, &choices("new"))
            .expect_err("directory rejected")
            .kind(),
        ErrorKind::InvalidInput
    );
    assert!(publish(&blocked, directory.path(), b"new file").is_err());
    assert_eq!(fs::read(blocked.join("marker")).expect("marker"), b"keep");
    assert_eq!(
        fs::read_dir(directory.path()).expect("directory").count(),
        2
    );
}

#[test]
fn concurrent_writers_publish_only_complete_coherent_files() {
    let directory = tempdir().expect("temporary settings");
    let path = directory.path().join("runtime-defaults.json");
    save_to(&path, &choices("initial")).expect("initial choices");
    let barrier = Barrier::new(5);
    let done = AtomicUsize::new(0);
    thread::scope(|scope| {
        for writer in 0..4 {
            let (path, barrier, done) = (&path, &barrier, &done);
            scope.spawn(move || {
                barrier.wait();
                for revision in 0..64 {
                    save_to(
                        path,
                        &choices(&format!("writer{writer}-revision{revision}")),
                    )
                    .expect("atomic save");
                }
                done.fetch_add(1, Ordering::Release);
            });
        }
        barrier.wait();
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut reads = 0;
        while done.load(Ordering::Acquire) != 4 || reads < 256 {
            assert!(
                Instant::now() < deadline,
                "writers finish within bounded test time"
            );
            let value = read_json(&path);
            assert!(value["model"].is_string());
            assert_eq!(value["model"], value["effort"]);
            assert_eq!(value["model"], value["summary"]);
            reads += 1;
        }
    });
    assert_eq!(
        fs::read_dir(directory.path())
            .expect("settings directory")
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn valid_symlink_preserves_the_link_and_atomically_updates_its_regular_target() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().expect("temporary settings");
    let target = directory.path().join("real/runtime-defaults.json");
    save_to(&target, &choices("before")).expect("initial target");
    let path = directory.path().join("runtime-defaults.json");
    symlink("real/runtime-defaults.json", &path).expect("relative symlink");
    save_to(&path, &choices("after")).expect("save through link");
    assert_eq!(
        fs::read_link(&path).expect("link preserved"),
        Path::new("real/runtime-defaults.json")
    );
    assert_eq!(load_from(&path), choices("after"));
    assert_eq!(load_from(&target), choices("after"));
    assert_eq!(
        fs::read_dir(target.parent().expect("parent"))
            .expect("target directory")
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn broken_and_directory_symlinks_remain_unchanged_after_save_errors() {
    use std::os::unix::fs::symlink;

    let directory = tempdir().expect("temporary settings");
    let path = directory.path().join("runtime-defaults.json");
    symlink("missing.json", &path).expect("broken link");
    let error = save_to(&path, &choices("new")).expect_err("broken link rejected");
    assert_eq!(error.kind(), ErrorKind::NotFound);
    assert!(error.to_string().contains("resolve runtime defaults link"));
    assert_eq!(
        fs::read_link(&path).expect("link remains"),
        Path::new("missing.json")
    );
    assert!(!directory.path().join("missing.json").exists());
    fs::remove_file(&path).expect("remove only broken test link");
    symlink(".", &path).expect("directory link");
    assert_eq!(
        save_to(&path, &choices("new"))
            .expect_err("directory rejected")
            .kind(),
        ErrorKind::InvalidInput
    );
    assert_eq!(fs::read_link(&path).expect("link remains"), Path::new("."));
    assert_eq!(
        fs::read_dir(directory.path()).expect("directory").count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn a_symlink_to_a_socket_is_rejected_without_touching_the_socket() {
    use std::os::unix::fs::{symlink, FileTypeExt};
    use std::os::unix::net::UnixListener;

    let directory = tempdir().expect("temporary settings");
    let target = directory.path().join("socket");
    let _listener = UnixListener::bind(&target).expect("socket fixture");
    let path = directory.path().join("runtime-defaults.json");
    symlink("socket", &path).expect("socket symlink");
    let error = save_to(&path, &choices("new")).expect_err("socket target rejected");
    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert_eq!(
        fs::read_link(&path).expect("link preserved"),
        Path::new("socket")
    );
    assert!(fs::symlink_metadata(&target)
        .expect("socket remains")
        .file_type()
        .is_socket());
    assert_eq!(
        fs::read_dir(directory.path()).expect("directory").count(),
        2
    );
}

fn choices(value: &str) -> RuntimeDefaults {
    RuntimeDefaults {
        model: Some(value.into()),
        effort: Some(value.into()),
        summary: Some(value.into()),
    }
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("complete settings file")).expect("complete JSON")
}
