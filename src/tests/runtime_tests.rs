//! Runtime subsystem test tree: CLI settings, ralph mode, picker, local history.

use std::sync::{Mutex, OnceLock};

use super::{
    detect_osc52_wrap, is_ssh_session, json, load_claude_thread_summaries,
    load_runtime_defaults_from, osc52_sequences_for_env, params_thread_archive, params_turn_start,
    parse_cli_args, parse_thread_list, parse_thread_runtime_settings, perf_session,
    persist_runtime_defaults_to, resolve_initial_runtime_settings, resume_hint,
    sort_threads_for_picker, styled_resume_hint, AppState, Backend, Message, MessageKind,
    Osc52Wrap, Role, RuntimeDefaults, ThreadSummary,
};

fn runtime_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

include!("runtime_tests/cli_runtime_settings.rs");

include!("runtime_tests/ralph_runtime.rs");

include!("runtime_tests/picker_and_local_history.rs");
