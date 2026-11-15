//! Read a bounded model catalog at the same checked revision as shared settings facts.

use carlos_proto::commands::RpcError;
use carlos_proto::ids::SettingsRevision;
use carlos_proto::settings::{SettingsPage, MAX_MODELS_PER_PAGE};

use super::state::SessionState;

impl SessionState {
    /// Reject stale catalog positions before cloning any requested model records.
    pub(crate) fn settings_page(
        &self,
        expected: Option<&SettingsRevision>,
        offset: u32,
        limit: usize,
    ) -> Result<SettingsPage, RpcError> {
        let settings = self
            .events
            .settings
            .as_ref()
            .ok_or_else(|| invalid("settings have not been published"))?;
        if settings.revision != self.settings_revision() {
            return Err(invalid(
                "publish current settings before reading their model catalog",
            ));
        }
        if expected.is_some_and(|expected| expected != &settings.revision) {
            return Err(invalid(
                "settings changed; read the current catalog before saving this draft",
            ));
        }
        let models = self.runtime_settings().available_models();
        let start = offset as usize;
        if start > models.len() {
            return Err(invalid(
                "model catalog offset is outside the current catalog",
            ));
        }
        let end = start
            .saturating_add(limit.clamp(1, MAX_MODELS_PER_PAGE))
            .min(models.len());
        Ok(SettingsPage {
            session: self.session_id().clone(),
            seq: self.events.seq,
            settings: settings.clone(),
            offset,
            models: models[start..end].to_vec(),
            next: (end < models.len()).then_some(end as u32),
        })
    }
}

fn invalid(message: &str) -> RpcError {
    RpcError {
        code: -32012,
        message: message.into(),
    }
}
