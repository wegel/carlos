//! Matching legacy files fail clearly when they cannot be read completely.

use std::fs;

use serde_json::json;

use super::{preferred_codex_thread, read_rollout_file};

#[test]
fn ineligible_inherited_history_is_absent_but_malformed_or_oversized_history_is_an_error() {
    let path = std::env::temp_dir().join(format!("carlos-legacy-{}.jsonl", uuid::Uuid::new_v4()));
    fs::write(
        &path,
        format!(
            "{}\n",
            json!({"type":"session_meta","payload":{"forked_from_id":"parent"}})
        ),
    )
    .expect("inherited fixture");
    assert!(read_rollout_file(&path, "fixture")
        .expect("deliberate ineligibility")
        .is_none());
    fs::write(&path, "invalid JSON\n").expect("malformed fixture");
    assert!(read_rollout_file(&path, "fixture")
        .expect_err("cannot claim complete history")
        .to_string()
        .contains("Read complete legacy"));
    fs::write(&path, vec![b'x'; 8 * 1024 * 1024 + 1]).expect("oversized fixture");
    let error = read_rollout_file(&path, "fixture").expect_err("bounded legacy failure");
    assert!(format!("{error:#}").contains("exceeds 8 MiB"));
    fs::remove_file(path).expect("remove owned fixture");
}

#[test]
fn authoritative_public_pages_bypass_even_an_invalid_raw_history_identity() {
    let response = json!({"result":{"thread":{"id":"modern","turns":[],"future":42},"carlosHistorySource":"paged"}});
    let thread =
        preferred_codex_thread(&response.to_string(), "../invalid").expect("modern authority");
    assert_eq!(thread["future"], 42);
}
