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
