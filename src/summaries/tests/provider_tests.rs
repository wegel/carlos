//! Inspect the exact model input and private reader paths with a harmless fake executable.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use crate::summaries::config::Config;
use crate::summaries::provider;
use crate::summaries::{command_key, CommandLocation, Control, Request};

#[test]
fn codex_provider_sends_only_body_and_hint_but_retains_private_read_authority() {
    let root = tempfile::tempdir().expect("root");
    let scripts = root.path().join("scripts");
    fs::create_dir(&scripts).expect("scripts");
    let (config, capture) = fixture(root.path());
    let body = "python3 - <<'PY'\nprint('cd /literal && text')\nPY\n";
    let request = Request {
        key: command_key("raw native identity"),
        command: body.into(),
        location: CommandLocation {
            cwd: scripts.clone(),
            read_root: root.path().to_owned(),
            directory: Some("scripts".into()),
        },
        entry: "item".into(),
        generate: true,
        file_based: false,
    };
    let control = Arc::new(Control {
        enabled: AtomicBool::new(true),
        stopped: AtomicBool::new(false),
        pid: AtomicU32::new(0),
        epoch: AtomicU64::new(0),
    });
    provider::create(&config)
        .summarize(&request, &control, 0)
        .expect("fake label");
    let captured: Value =
        serde_json::from_slice(&fs::read(capture).expect("capture")).expect("JSON");
    assert_eq!(
        captured["input"],
        serde_json::json!({"command":body,"directory":"scripts"})
    );
    let args = captured["argv"].as_array().expect("arguments");
    let cwd_flag = args.iter().position(|arg| arg == "-C").expect("native cwd");
    assert_eq!(args[cwd_flag + 1], scripts.to_string_lossy().as_ref());
    let reader = args
        .iter()
        .filter_map(Value::as_str)
        .find(|arg| arg.starts_with("mcp_servers.source_reader.args="))
        .expect("reader args");
    let paths: Value =
        serde_json::from_str(reader.split_once('=').expect("setting").1).expect("paths");
    assert_eq!(paths[0], "__summary-reader");
    assert_eq!(paths[1], root.path().to_string_lossy().as_ref());
    assert_eq!(paths[2], scripts.to_string_lossy().as_ref());
}

fn fixture(root: &Path) -> (Config, PathBuf) {
    let state = root.join("state");
    fs::create_dir_all(state.join("summary-codex-home")).expect("state");
    fs::write(state.join("summary-codex-home/auth.json"), "{}").expect("fake auth");
    let peer = root.join("peer.py");
    fs::write(&peer, r#"import json,os,sys
from pathlib import Path
Path(sys.argv[1]).write_text(json.dumps({'input':json.load(sys.stdin),'argv':sys.argv[2:],'cwd':os.getcwd()}))
print(json.dumps({'type':'item.completed','item':{'type':'agent_message','text':'Read the fixture script'}}))
print(json.dumps({'type':'turn.completed'}))
"#).expect("peer");
    let capture = root.join("capture.json");
    let config = Config {
        executable: "python3".into(),
        arguments: vec![
            peer.to_string_lossy().into_owned(),
            capture.to_string_lossy().into_owned(),
        ],
        model: "fixture".into(),
        codex: true,
        timeout: Duration::from_secs(3),
        state,
    };
    (config, capture)
}
