//! Expose borrowed task facts and explicit native panel reset/install commands.

use std::collections::HashMap;
use std::time::Instant;

use serde_json::Value;

use super::panel_scope::PanelScope;
use super::panel_state::{PanelState, Process, VerifiedOwners};
use super::task_status::TaskStatus;
use crate::session::state::SessionState;

impl SessionState {
    /// Clear reports when the transcript replaces its source history.
    pub(crate) fn clear_codex_reports(&mut self) {
        self.codex_panel.reports.clear();
    }
    /// Clear the previous native panel failure after an explicit refresh.
    pub fn clear_codex_panel_error(&mut self) {
        self.codex_panel.error = None;
    }
    /// Borrow the native panel cache without exposing mutable readers.
    pub fn codex_panel(&self) -> &PanelState {
        &self.codex_panel
    }
    /// Borrow targeted process-read facts without exposing pending requests.
    pub fn codex_task_status(&self) -> &TaskStatus {
        &self.codex_task_status
    }
    /// Forget the old root's panel facts after a thread switch.
    pub(crate) fn reset_codex_panel(&mut self) {
        self.codex_panel = Default::default();
    }
    /// Forget targeted readers belonging to a replaced native root.
    pub(crate) fn reset_codex_task_status(&mut self) {
        self.codex_task_status = Default::default();
    }
    /// Drop the previous selected-child read after explicit client navigation.
    pub fn clear_codex_panel_activity(&mut self) {
        self.codex_panel.activities.clear();
    }
    /// Release only one child's read after its last explicit client interest ends.
    pub fn forget_codex_child(&mut self, thread: &str) {
        if let Some(activity) = self.codex_panel.activities.remove(thread) {
            if let Some(request) = activity.request {
                self.codex_panel
                    .outgoing
                    .retain(|pending| pending.token != request.token);
            }
        }
    }
    /// Fence pending native panel controls after connection closure.
    pub(crate) fn close_codex_panel(&mut self) {
        self.codex_panel.outgoing.clear();
        self.codex_panel.snapshot = None;
        self.codex_panel.stop = None;
        self.codex_panel.error = Some("Codex closed; pending panel work was not confirmed".into());
        for activity in self.codex_panel.activities.values_mut() {
            activity.request = None;
        }
    }
    /// Install already validated reconnect task facts, retaining each native owner's identity.
    pub(crate) fn install_codex_panel(
        &mut self,
        panel: &Value,
        ownership: VerifiedOwners,
        scopes: HashMap<String, Option<String>>,
        scope: PanelScope<'_>,
    ) {
        self.codex_panel.ownership = ownership;
        self.codex_panel.process_scopes = scopes;
        self.codex_task_status = Default::default();
        self.codex_panel.agents = panel["agents"].as_array().cloned().unwrap_or_default();
        self.codex_panel.processes = panel["processes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|row| {
                Some(Process {
                    thread: row["thread"].as_str()?.into(),
                    data: row["data"].clone(),
                })
            })
            .collect();
        self.codex_panel.older = panel["older"].clone();
        self.codex_panel.older_window = false;
        self.codex_panel.activities.clear();
        self.codex_panel.refreshed = Some(Instant::now());
        self.codex_panel.error = panel["error"].as_str().map(str::to_string);
        self.codex_panel.notice = panel["notice"]
            .as_str()
            .unwrap_or("Agent data unavailable on this server")
            .into();
        super::task_status::panel_updated(self, scope);
    }
}
