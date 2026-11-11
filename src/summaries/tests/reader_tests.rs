//! Enforce native source-tool limits without importing or executing inspected code.

use std::fs;

use crate::summaries::reader::Reader;

#[test]
fn native_reader_returns_text_and_hashes_without_execution() {
    let root = tempfile::tempdir().expect("tempdir");
    let workspace = root.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    let text = "raise RuntimeError('must never execute')\n";
    fs::write(workspace.join("process.py"), text).expect("source");
    let audit = root.path().join("audit.jsonl");
    let mut reader = Reader::new(&workspace, &workspace, &audit).expect("reader");
    assert_eq!(reader.call("read_file", "process.py").expect("read"), text);
    let record: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(audit).expect("audit")).expect("record");
    assert_eq!(
        record["hash"],
        blake3::hash(text.as_bytes()).to_hex().to_string()
    );
    assert!(reader.call("execute", "process.py").is_err());
    assert!(reader.call("read_file", "../audit.jsonl").is_err());
}

#[test]
fn native_reader_denies_oversize_source_and_excess_calls() {
    let root = tempfile::tempdir().expect("tempdir");
    fs::write(root.path().join("large.py"), vec![b'x'; 65_537]).expect("source");
    fs::write(root.path().join("small.py"), "print(1)").expect("source");
    let mut reader =
        Reader::new(root.path(), root.path(), &root.path().join("audit.jsonl")).expect("reader");
    assert!(reader.call("read_file", "large.py").is_err());
    for _ in 0..3 {
        assert!(reader.call("read_file", "small.py").is_ok());
    }
    assert!(reader.call("read_file", "small.py").is_err());
}

#[cfg(unix)]
#[test]
fn native_reader_denies_symlinks_outside_the_workspace() {
    let root = tempfile::tempdir().expect("tempdir");
    let workspace = root.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace");
    fs::write(root.path().join("outside.py"), "private source").expect("source");
    std::os::unix::fs::symlink(root.path().join("outside.py"), workspace.join("escape.py"))
        .expect("link");
    let mut reader =
        Reader::new(&workspace, &workspace, &root.path().join("audit.jsonl")).expect("reader");
    assert!(reader.call("read_file", "escape.py").is_err());
}

#[test]
fn reader_resolves_from_effective_cwd_but_audits_siblings_relative_to_original_authority() {
    let root = tempfile::tempdir().expect("root");
    let cwd = root.path().join("scripts");
    fs::create_dir(&cwd).expect("scripts");
    fs::write(root.path().join("shared.py"), "SHARED_ROOT_SOURCE").expect("shared");
    fs::write(cwd.join("local.py"), "EFFECTIVE_DIRECTORY_SOURCE").expect("local");
    let audit = root.path().join("audit.jsonl");
    let mut reader = Reader::new(root.path(), &cwd, &audit).expect("bounded reader");
    assert_eq!(
        reader.call("read_file", "../shared.py").expect("sibling"),
        "SHARED_ROOT_SOURCE"
    );
    assert_eq!(
        reader.call("read_file", "local.py").expect("effective cwd"),
        "EFFECTIVE_DIRECTORY_SOURCE"
    );
    let paths: Vec<_> = fs::read_to_string(audit)
        .expect("audit")
        .lines()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).expect("record")["path"].clone()
        })
        .collect();
    assert_eq!(
        paths,
        vec![
            serde_json::json!("shared.py"),
            serde_json::json!("scripts/local.py")
        ]
    );
    let outside = tempfile::tempdir().expect("outside");
    assert!(Reader::new(
        root.path(),
        outside.path(),
        &root.path().join("outside-audit")
    )
    .is_err());
}

#[cfg(unix)]
#[test]
fn reader_rejects_a_symlinked_effective_directory_outside_the_original_root() {
    let root = tempfile::tempdir().expect("root");
    let outside = tempfile::tempdir().expect("outside");
    let link = root.path().join("scripts");
    std::os::unix::fs::symlink(outside.path(), &link).expect("directory link");
    assert!(Reader::new(root.path(), &link, &root.path().join("audit")).is_err());
}
