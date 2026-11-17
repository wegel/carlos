//! Keep saved listener settings explicit, private and bounded.

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::PathBuf;

use tempfile::TempDir;

fn configuration(body: &str) -> (TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("private fixture directory");
    let path = directory.path().join("daemon.json");
    fs::write(&path, body).expect("fixture configuration");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("private file");
    (directory, path)
}

#[test]
fn absent_settings_keep_network_access_disabled() {
    let settings = super::read(None).expect("no selected file");
    assert!(settings.listen.is_none());
    assert!(settings.allow_origin.is_none());
}

#[test]
fn a_selected_file_retains_the_exact_interface_and_origin() {
    let (_directory, path) =
        configuration(r#"{"listen":"127.0.0.1:33732","allow_origin":"http://localhost:3000"}"#);
    let settings = super::read(Some(&path)).expect("explicit listener settings");
    assert_eq!(
        settings.listen.expect("listener").to_string(),
        "127.0.0.1:33732"
    );
    assert_eq!(
        settings.allow_origin.as_deref(),
        Some("http://localhost:3000")
    );
}

#[test]
fn malformed_unknown_duplicate_and_oversized_settings_fail() {
    for body in [
        r#"{"listen":"localhost:33732"}"#.to_owned(),
        r#"{"listne":"127.0.0.1:33732"}"#.to_owned(),
        r#"{"listen":null,"listen":"127.0.0.1:33732"}"#.to_owned(),
        " ".repeat(4097),
    ] {
        let (_directory, path) = configuration(&body);
        assert!(
            super::read(Some(&path)).is_err(),
            "invalid settings: {body}"
        );
    }
}

#[test]
fn a_shared_file_or_symlink_cannot_enable_a_listener() {
    let (directory, path) = configuration(r#"{"listen":"127.0.0.1:33732"}"#);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("shared fixture file");
    assert!(super::read(Some(&path)).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("private file");
    let alias = directory.path().join("alias.json");
    symlink(&path, &alias).expect("fixture symlink");
    assert!(super::read(Some(&alias)).is_err());
}

#[test]
fn a_missing_selected_file_fails_instead_of_silently_disabling_the_listener() {
    let directory = tempfile::tempdir().expect("private fixture directory");
    assert!(super::read(Some(&directory.path().join("missing.json"))).is_err());
}
