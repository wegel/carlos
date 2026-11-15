//! Keep timestamp and cwd-filtered saved metadata reads inside the Grok backend.

use std::fs;
use std::path::Path;

use super::{load_grok_thread_summaries_from, parse_iso_timestamp_secs};

#[test]
fn parse_iso_timestamp_secs_reads_rfc3339_prefix() {
    assert_eq!(
        parse_iso_timestamp_secs("2026-08-17T13:00:47.326462776+00:00"),
        1_786_971_647
    );
}

#[test]
fn load_grok_local_history_filters_by_cwd() {
    let directory = tempfile::tempdir().expect("private native session summaries");
    let root = directory.path().to_path_buf();
    let cwd = Path::new("/repo");
    let other = Path::new("/other");
    let group = root.join("%2Frepo");
    let keep = group.join("session-keep");
    let skip = group.join("session-skip");
    fs::create_dir_all(&keep).expect("keep dir");
    fs::create_dir_all(&skip).expect("skip dir");
    fs::write(
        keep.join("summary.json"),
        r#"{
            "info": { "id": "session-keep", "cwd": "/repo" },
            "session_summary": "Keep me",
            "created_at": "2026-08-17T12:00:00Z",
            "updated_at": "2026-08-17T13:00:00Z"
        }"#,
    )
    .expect("write keep");
    fs::write(
        skip.join("summary.json"),
        r#"{
            "info": { "id": "session-skip", "cwd": "/other" },
            "session_summary": "Skip me",
            "created_at": "2026-08-17T12:00:00Z",
            "updated_at": "2026-08-17T14:00:00Z"
        }"#,
    )
    .expect("write skip");

    let listed = load_grok_thread_summaries_from(root.clone(), cwd).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].session_id, "session-keep");
    assert_eq!(listed[0].title.as_deref(), Some("Keep me"));

    let other_listed = load_grok_thread_summaries_from(root.clone(), other).expect("list other");
    assert_eq!(other_listed.len(), 1);
    assert_eq!(other_listed[0].session_id, "session-skip");
}
