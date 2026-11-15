//! Claude backend test tree: CLI handshake, history recovery, translation, and failures.

use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use carlos_core::backend::{BackendClient, EventReceiver};
use serde_json::Value;

use super::{parse_cli_args, AppState, Backend};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

include!("claude_backend_tests/cli_and_session_history.rs");

include!("claude_backend_tests/review_receiver_tests.rs");
