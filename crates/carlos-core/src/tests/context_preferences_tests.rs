//! Parse, bound, and round-trip context choices without modifying native CLI files.

use std::fs;

use super::{label, load_from, parse, save_to};

#[test]
fn sizes_accept_exact_counts_suffixes_and_auto_but_reject_overflow_or_bad_ranges() {
    for (text, tokens) in [
        ("Auto", None),
        (" 500k ", Some(500_000)),
        ("1M", Some(1_000_000)),
        ("123456", Some(123_456)),
    ] {
        assert_eq!(parse(text).expect("supported size"), tokens);
        assert_eq!(parse(&label(tokens)).expect("displayed size"), tokens);
    }
    for text in [
        "",
        "99k",
        "1001k",
        "-200000",
        "1.5M",
        "9999999999999999999999999k",
        "200k garbage",
    ] {
        assert!(parse(text).is_err(), "{text}");
    }
}

#[test]
fn saved_sizes_round_trip_and_invalid_files_do_not_silently_become_auto() {
    let directory = tempfile::tempdir().expect("isolated preferences");
    let path = directory.path().join("carlos/context.json");
    assert_eq!(load_from(&path).expect("missing file"), None);
    save_to(&path, Some(500_000)).expect("save exact size");
    assert_eq!(load_from(&path).expect("read exact size"), Some(500_000));
    save_to(&path, None).expect("save Auto");
    assert_eq!(load_from(&path).expect("read Auto"), None);
    for text in [
        "broken JSON",
        "{}",
        "{\"tokens\":-1}",
        "{\"tokens\":\"500k\"}",
    ] {
        fs::write(&path, text).expect("malformed fixture");
        assert!(load_from(&path).is_err(), "{text}");
    }
}

#[cfg(unix)]
#[test]
fn context_save_preserves_a_valid_link_and_rejects_a_broken_one_without_changes() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().expect("isolated preferences");
    let target = directory.path().join("real.json");
    save_to(&target, Some(123_456)).expect("initial target");
    let link = directory.path().join("context.json");
    symlink("real.json", &link).expect("relative link");
    save_to(&link, Some(500_000)).expect("save through link");
    assert_eq!(
        fs::read_link(&link).expect("link preserved"),
        std::path::Path::new("real.json")
    );
    assert_eq!(load_from(&target).expect("updated target"), Some(500_000));
    fs::remove_file(&target).expect("make link broken");
    let error = save_to(&link, None).expect_err("broken link rejected");
    assert_eq!(error.to_string(), "Save Carlos context settings");
    assert_eq!(
        fs::read_link(&link).expect("broken link preserved"),
        std::path::Path::new("real.json")
    );
    assert!(!target.exists());
    assert_eq!(
        fs::read_dir(directory.path()).expect("directory").count(),
        1
    );
}

#[test]
fn a_failed_context_publish_keeps_the_old_directory_and_error_context() {
    let directory = tempfile::tempdir().expect("isolated preferences");
    let target = directory.path().join("context.json");
    fs::create_dir(&target).expect("directory target");
    fs::write(target.join("marker"), "keep").expect("prior contents");
    let error = save_to(&target, Some(500_000)).expect_err("cannot replace directory");
    assert_eq!(error.to_string(), "Save Carlos context settings");
    assert_eq!(
        fs::read_to_string(target.join("marker")).expect("prior marker"),
        "keep"
    );
    assert_eq!(
        fs::read_dir(directory.path()).expect("directory").count(),
        1
    );
}
