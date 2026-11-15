//! Seed bounded native task fixtures without exposing mutable session records.

use std::time::Instant;

use serde_json::Value;

use super::panel_state::{Process, VerifiedOwners};
use crate::session::state::SessionState;

impl SessionState {
    /// Set the fixture load result for one native process owner.
    pub fn set_codex_process_scope_for_test(&mut self, owner: String, error: Option<String>) {
        self.codex_panel.process_scopes.insert(owner, error);
    }
    /// Append a fixture native child-agent record.
    pub fn push_codex_agent_for_test(&mut self, agent: Value) {
        self.codex_panel.agents.push(agent);
    }
    /// Replace the fixture native child-agent records.
    pub fn set_codex_agents_for_test(&mut self, agents: Vec<Value>) {
        self.codex_panel.agents = agents;
    }
    /// Append a fixture native background-process record.
    pub fn push_codex_process_for_test(&mut self, process: Process) {
        self.codex_panel.processes.push(process);
    }
    /// Replace the fixture native background-process records.
    pub fn set_codex_processes_for_test(&mut self, processes: Vec<Process>) {
        self.codex_panel.processes = processes;
    }
    /// Remove all fixture native background-process records.
    pub fn clear_codex_processes_for_test(&mut self) {
        self.codex_panel.processes.clear();
    }
    /// Mark all fixture child agents as verified for the supplied connection epoch.
    pub fn verify_codex_agent_owners_for_test(&mut self, epoch: u64) {
        self.codex_panel.ownership = VerifiedOwners::new(
            &self.thread_id,
            epoch,
            self.codex_panel
                .agents
                .iter()
                .map(|agent| agent["id"].as_str().expect("agent ID").to_string()),
        )
        .expect("verified test owners");
    }
    /// Set the fixture byte count retained for native panel snapshots.
    pub fn set_codex_panel_snapshot_bytes_for_test(&mut self, bytes: usize) {
        self.codex_panel.snapshot.as_mut().expect("read").bytes = bytes;
    }
    /// Set when the fixture saved the named native panel snapshot.
    pub fn set_codex_panel_snapshot_time_for_test(&mut self, at: Instant) {
        self.codex_panel
            .snapshot
            .as_mut()
            .expect("snapshot")
            .started = at;
    }
    /// Set when the fixture observed the named native task.
    pub fn set_codex_panel_activity_time_for_test(&mut self, at: Instant) {
        self.codex_panel
            .activities
            .values_mut()
            .next()
            .expect("activity")
            .started = at;
    }
    /// Set the fixture deadline for refreshing native panel rows.
    pub fn set_codex_panel_refresh_time_for_test(&mut self, at: Instant) {
        self.codex_panel.refreshed = Some(at);
    }
    /// Mark whether the fixture displays older native task rows.
    pub fn set_codex_panel_older_window_for_test(&mut self, older: bool) {
        self.codex_panel.older_window = older;
    }
    /// Record a fixture failure for the selected native task owner.
    pub fn seed_codex_task_error_for_test(&mut self, owner: &str, error: &str) {
        self.codex_task_status
            .errors
            .insert(owner.into(), error.into());
        self.codex_task_status.uncertain.insert(owner.into());
    }
}
