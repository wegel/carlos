//! Sends native main-turn interrupts without claiming queued prompts were canceled.

use std::time::{Duration, Instant};

use super::state::AppState;
use super::Role;
use crate::backend::{BackendClient, BackendKind};
use crate::protocol_params::params_turn_interrupt;

/// Interrupt main work while preserving native-accepted queued prompt ownership.
pub(super) fn interrupt_active_turn(client: &dyn BackendClient, app: &mut AppState) {
    let Some(turn_id) = app.active_turn_id.clone().or_else(|| {
        (client.kind() == BackendKind::Claude
            && app.claude.activity.state == super::claude_activity::NativeState::Running)
            .then(|| "native-main".to_string())
    }) else {
        return;
    };
    let params = params_turn_interrupt(&app.thread_id, &turn_id);
    let interrupted = if client.kind() == BackendKind::Claude {
        app.claude.begin_interrupt()
    } else {
        Vec::new()
    };
    match client.call("turn/interrupt", params, Duration::from_secs(10)) {
        Ok(response) => {
            if client.kind() == BackendKind::Claude {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&response) {
                    if let Some(ids) = value["result"]["still_queued"]
                        .as_array()
                        .filter(|ids| !ids.is_empty())
                    {
                        app.append_message(Role::System,format!("Main-turn interrupt requested; {} accepted queued prompt(s) remain with Claude and will still run",ids.len()));
                    }
                }
            }
            app.append_turn_interrupted_marker();
            app.set_status("interrupt requested");
        }
        Err(e) => {
            app.claude.rollback_interrupt(&interrupted);
            if client.kind() == BackendKind::Claude {
                app.ralph_runtime.stop_after_failure();
            }
            app.set_status(format!("{e}"));
        }
    }
    app.suppress_esc_chord_after_interrupt(Instant::now());
}
