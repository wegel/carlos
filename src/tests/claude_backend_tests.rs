//! Keep the remaining plain Claude CLI flag and environment-default checks.

use std::sync::{Mutex, OnceLock};

use super::{parse_cli_args, Backend};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

include!("claude_backend_tests/cli_and_session_history.rs");
