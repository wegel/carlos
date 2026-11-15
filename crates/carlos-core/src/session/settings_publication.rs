//! Publish native choices without hashing the model catalog on transcript-only changes.

use carlos_proto::ids::SettingsRevision;
use carlos_proto::session::Backend;
use carlos_proto::settings::{ModelInfo, RuntimeChoices, SettingsFacts};

use super::events::Change;
use super::state::SessionState;
use super::task_digest::hash_text;
use crate::backend::BackendKind;

pub(super) fn catalog_hash(models: &[ModelInfo]) -> blake3::Hash {
    let mut hash = blake3::Hasher::new();
    for model in models {
        hash_text(&mut hash, &model.model);
        hash_text(&mut hash, &model.display_name);
        hash.update(&[u8::from(model.is_default)]);
        hash_option(&mut hash, model.default_effort.as_deref());
        hash.update(&(model.supported_efforts.len() as u64).to_le_bytes());
        for effort in &model.supported_efforts {
            hash_text(&mut hash, effort);
        }
    }
    hash.finalize()
}

impl SessionState {
    pub(super) fn publish_settings(&mut self, changes: &mut Vec<Change>) {
        let revision = self.settings_revision();
        if self
            .events
            .settings
            .as_ref()
            .is_some_and(|facts| facts.revision == revision)
        {
            return;
        }
        let runtime = self.runtime_settings();
        self.events.settings = Some(SettingsFacts {
            revision,
            current: RuntimeChoices {
                model: runtime.current_model().clone(),
                effort: runtime.current_effort().clone(),
                summary: runtime.current_summary().clone(),
            },
            pending: RuntimeChoices {
                model: runtime.pending_model().clone(),
                effort: runtime.pending_effort().clone(),
                summary: runtime.pending_summary().clone(),
            },
            supports_effort: runtime.supports_effort(),
            supports_summary: runtime.supports_summary(),
            model_count: runtime.available_models().len() as u32,
            native_pending: self.native_settings_pending(),
            legacy_overrides: self.codex_settings().legacy_overrides(),
            label: self.settings_label(),
            summaries_enabled: self.summaries_enabled(),
        });
        changes.push(Change::Settings);
    }

    /// Fence settings reads and saves against a changed backend, catalog or accepted choice.
    pub fn settings_revision(&self) -> SettingsRevision {
        let mut hash = blake3::Hasher::new();
        hash.update(b"carlos.settings.v1");
        hash.update(self.events.owner.as_bytes());
        hash_text(&mut hash, self.session_id().as_str());
        hash_text(&mut hash, self.thread_id());
        let (kind, tag) = match self.events.backend {
            Backend::Codex => (BackendKind::Codex, 0),
            Backend::Claude => (BackendKind::Claude, 1),
            Backend::Grok => (BackendKind::Grok, 2),
        };
        let (epoch, generation) = self.command_connection(kind);
        hash.update(&[tag, u8::from(self.events.backend_closed)]);
        hash.update(&epoch.to_le_bytes());
        hash.update(&generation.to_le_bytes());
        hash.update(self.runtime.catalog_hash.as_bytes());
        for choice in [
            self.runtime.current_model(),
            self.runtime.current_effort(),
            self.runtime.current_summary(),
            self.runtime.pending_model(),
            self.runtime.pending_effort(),
            self.runtime.pending_summary(),
        ] {
            hash_option(&mut hash, choice.as_deref());
        }
        hash.update(&[
            u8::from(self.claude.inherits_model),
            u8::from(self.claude.inherits_effort),
            u8::from(self.runtime.supports_effort()),
            u8::from(self.runtime.supports_summary()),
            u8::from(self.native_settings_pending()),
            u8::from(self.codex_settings().legacy_overrides()),
            u8::from(self.summaries_enabled()),
        ]);
        hash.update(&self.codex_settings().revision().to_le_bytes());
        hash_text(&mut hash, &self.settings_label());
        hash.finalize()
            .to_hex()
            .as_str()
            .parse()
            .expect("settings digest")
    }

    fn settings_label(&self) -> String {
        if self.events.backend == Backend::Codex {
            super::codex::settings::label(self)
        } else {
            self.runtime_settings_label()
        }
    }

    fn native_settings_pending(&self) -> bool {
        self.runtime.native_settings_pending
            || self.codex_settings.pending.is_some()
            || self.codex_settings.deferred.is_some()
    }
}

fn hash_option(hash: &mut blake3::Hasher, value: Option<&str>) {
    hash.update(&[u8::from(value.is_some())]);
    if let Some(value) = value {
        hash_text(hash, value);
    }
}
