//! Sends native main-turn interrupts without claiming queued prompts were canceled.

use std::time::Duration;

use super::item_changes::ItemChanges;
use super::state::SessionState;
use crate::backend::{BackendClient, BackendKind};
use crate::protocol_params::params_turn_interrupt;
use crate::session::models::Role;

/// Interrupt main work while preserving native-accepted queued prompt ownership.
impl SessionState {
    /// Interrupt accepted main work without discarding native queued prompts.
    pub fn interrupt_active_turn(&mut self, client: &dyn BackendClient) -> ItemChanges {
        let mut changes = ItemChanges::default();
        let Some(turn_id) = self.active_turn_id().clone().or_else(|| {
            (client.kind() == BackendKind::Claude && self.claude().activity().is_running())
                .then(|| "native-main".to_string())
        }) else {
            return changes;
        };
        let params = params_turn_interrupt(self.thread_id(), &turn_id);
        match client.call("turn/interrupt", params, Duration::from_secs(10)) {
            Ok(response) => {
                if client.kind() == BackendKind::Claude {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&response) {
                        if let Some(ids) = value["result"]["still_queued"]
                            .as_array()
                            .filter(|ids| !ids.is_empty())
                        {
                            changes.track(self.append_message(Role::System,format!("Main-turn interrupt requested; {} accepted queued prompt(s) remain with Claude and will still run",ids.len())));
                        }
                    }
                }
                changes.push(self.append_turn_interrupted_marker());
                self.set_status("interrupt requested");
            }
            Err(e) => {
                if client.kind() == BackendKind::Claude {
                    self.ralph_stop_after_failure();
                }
                self.set_status(format!("{e}"));
            }
        }
        changes
    }
}
