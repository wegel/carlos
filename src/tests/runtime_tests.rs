//! Runtime subsystem test tree: CLI settings, ralph mode, picker, local history.

use carlos_core::session::models::MessageKind;
use carlos_core::session::models::Role;
use carlos_core::session::runtime_settings::RuntimeDefaults;

use super::{
    detect_osc52_wrap, is_ssh_session, json, osc52_sequences_for_env, params_thread_archive,
    params_turn_start, parse_cli_args, parse_thread_list, parse_thread_runtime_settings, Backend,
    Osc52Wrap,
};

include!("runtime_tests/cli_runtime_settings.rs");

include!("runtime_tests/ralph_marker_tests.rs");

include!("runtime_tests/picker_and_local_history.rs");

use carlos_core::runtime_preferences::{
    load_from as load_runtime_defaults_from, save_to as persist_runtime_defaults_to,
};
