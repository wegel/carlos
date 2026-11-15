//! Check the actual daemon executable's private source-reader mode without a model or terminal.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

#[test]
fn source_reader_reads_only_the_requested_workspace_and_never_executes_source() {
    let root = tempfile::tempdir().expect("reader workspace");
    let workspace = root.path().join("workspace");
    std::fs::create_dir(&workspace).expect("source directory");
    let text = "raise RuntimeError('source must never execute')\n";
    std::fs::write(workspace.join("source.py"), text).expect("source fixture");
    std::fs::write(root.path().join("outside.py"), "private outside source")
        .expect("outside fixture");
    let audit = root.path().join("audit.jsonl");
    let mut child = Command::new(env!("CARGO_BIN_EXE_carlosd"))
        .arg("__summary-reader")
        .arg(&workspace)
        .arg(&workspace)
        .arg(&audit)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("real daemon reader");
    let mut input = child.stdin.take().expect("reader stdin");
    for request in requests() {
        writeln!(input, "{request}").expect("reader request");
    }
    drop(input);
    let output = child.wait_with_output().expect("reader exits at stdin EOF");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    check_replies(output.stdout, text);
    let audit: Value = serde_json::from_str(&std::fs::read_to_string(audit).expect("read audit"))
        .expect("audit JSON");
    assert!(audit["hash"].as_str().is_some_and(|hash| hash.len() == 64));
}

fn requests() -> [Value; 5] {
    [
        json!({"id":1,"method":"initialize","params":{"protocolVersion":"fixture"}}),
        json!({"id":2,"method":"tools/list","params":{}}),
        json!({"id":3,"method":"tools/call","params":{"name":"read_file","arguments":{"path":"source.py"}}}),
        json!({"id":4,"method":"tools/call","params":{"name":"read_file","arguments":{"path":"../outside.py"}}}),
        json!({"id":5,"method":"tools/call","params":{"name":"execute","arguments":{"path":"source.py"}}}),
    ]
}

fn check_replies(bytes: Vec<u8>, text: &str) {
    let replies: Vec<Value> = String::from_utf8(bytes)
        .expect("UTF8 replies")
        .lines()
        .map(|line| serde_json::from_str(line).expect("JSON reply"))
        .collect();
    assert_eq!(replies.len(), 5);
    assert_eq!(
        replies[0]["result"]["serverInfo"]["name"],
        "carlos-source-reader"
    );
    let tools = replies[1]["result"]["tools"]
        .as_array()
        .expect("readonly tool list");
    assert_eq!(tools.len(), 2);
    assert!(tools
        .iter()
        .all(|tool| tool["annotations"]["readOnlyHint"] == true));
    assert_eq!(replies[2]["result"]["content"][0]["text"], text);
    assert_eq!(replies[3]["result"]["isError"], true);
    assert_eq!(replies[4]["result"]["isError"], true);
}
