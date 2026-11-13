//! Connect fixed native ACP frames to the real Grok process reader without an account.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};

use serde_json::Value;

use super::process::start_command;
use super::{grok_model_catalog, GrokClient, GrokRuntimeSettings, GrokTranslationState};
use crate::backend::EventReceiver;

/// Return a client whose normal OS-pipe reader receives only the supplied fixture frames.
pub(crate) fn client(frames: Vec<Value>) -> (GrokClient, EventReceiver) {
    let (events_tx, events_rx) = mpsc::channel();
    let translation = Arc::new(Mutex::new(GrokTranslationState::default()));
    let process = start_command(peer(frames), events_tx.clone(), Arc::clone(&translation))
        .expect("fixed Grok peer");
    let client = GrokClient {
        cwd: PathBuf::from("."),
        session_id: Mutex::new(Some("root".into())),
        runtime_settings: Mutex::new(GrokRuntimeSettings::default()),
        models: Mutex::new(grok_model_catalog()),
        translation,
        process: Mutex::new(process),
        events_tx,
        events_rx: None,
    };
    (client, events_rx.into())
}

fn peer(frames: Vec<Value>) -> Command {
    let mut command = Command::new("python3");
    command.args([
        "-u",
        "-c",
        concat!(
            "import sys,json\n",
            "frames=json.loads(sys.argv[1])\n",
            "for line in sys.stdin:\n",
            " r=json.loads(line)\n",
            " if r.get('method')=='session/prompt':\n",
            "  for frame in frames: print(json.dumps(frame),flush=True)\n",
            " print(json.dumps({'id':r['id'],'result':{'stopReason':'end_turn'}}),flush=True)\n"
        ),
        &serde_json::to_string(&frames).expect("frames"),
    ]);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}
