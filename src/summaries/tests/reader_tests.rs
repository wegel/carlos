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
    let mut reader = Reader::new(&workspace, &audit).expect("reader");
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
    let mut reader = Reader::new(root.path(), &root.path().join("audit.jsonl")).expect("reader");
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
    let mut reader = Reader::new(&workspace, &root.path().join("audit.jsonl")).expect("reader");
    assert!(reader.call("read_file", "escape.py").is_err());
}
