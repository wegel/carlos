//! Adapt shared model lists to private terminal choices without owning page requests.

use anyhow::Result;
use carlos_client::panels::PanelKind;
use carlos_client::Effects;
use carlos_core::session::runtime_settings::RuntimeDefaults;
use carlos_proto::settings::SettingsChange;
use carlos_proto::submissions::ClientOperation;

use super::transport::Transport;
use super::Device;
use crate::app::runtime_settings_state::RuntimeSettingsRef;

pub(super) use carlos_client::panels::SettingsCatalog as Catalog;

pub(super) fn catalog_runtime(catalog: &Catalog) -> RuntimeSettingsRef<'_> {
    RuntimeSettingsRef::from((&catalog.facts, catalog.models.as_slice()))
}

#[derive(Default)]
pub(super) struct Settings {
    pub catalog: Option<Catalog>,
    pub error: Option<String>,
    pub initial: Option<RuntimeDefaults>,
    pub operation: Option<ClientOperation>,
    pub submitted: Option<super::settings_intent::SaveIntent>,
    pub appearance_save: Option<super::settings_appearance::Save>,
    pub context: super::settings_context::Context,
    pub keep_draft: bool,
    restore: Option<SettingsChange>,
}

impl Device {
    pub(super) fn open_settings(&mut self, link: Option<&mut Transport>) -> Result<()> {
        self.view.runtime_settings.show_model_settings = true;
        if self.settings.submitted.is_some()
            || self.settings.catalog.is_some() && self.settings.keep_draft
        {
            return Ok(());
        }
        let appearance_save = self.settings.appearance_save.take();
        let mut context = std::mem::take(&mut self.settings.context);
        context.reopen();
        self.view.runtime_settings.context = Default::default();
        self.settings = Settings {
            appearance_save,
            context,
            ..Default::default()
        };
        let effects = self.client.open_settings_panel(&self.session)?;
        if let Some(link) = link {
            self.apply_effects(link, effects)?;
            self.drive_settings(link)?;
        } else {
            self.settings.error = Some("Disconnected; F5 reconnects. Your draft is kept".into());
        }
        Ok(())
    }

    pub(super) fn apply_settings_reads(&mut self, effects: &mut Effects) -> bool {
        let mut changed = self.apply_context_reads(effects);
        if effects
            .panels
            .iter()
            .any(|change| change.session == self.session && change.kind == PanelKind::Settings)
        {
            if let Some(catalog) = self.client.take_settings_catalog(&self.session) {
                self.install_settings_catalog(catalog);
            }
            changed = true;
        }
        changed
    }

    /// Drive only this terminal's save choices and host context editor; Client owns model pages.
    pub(super) fn drive_settings(&mut self, link: &mut Transport) -> Result<bool> {
        let changed = self.drive_settings_save(link)?;
        if self.view.runtime_settings.show_model_settings {
            self.drive_settings_context(link)?;
        }
        Ok(changed)
    }

    fn install_settings_catalog(&mut self, catalog: Catalog) {
        let refreshing = self.settings.catalog.is_some();
        let selected = self.view.runtime_settings.model_settings_field;
        let mode = self.view.runtime_settings.model_settings_tool_output;
        let context = std::mem::take(&mut self.view.runtime_settings.context);
        self.view
            .runtime_settings
            .open_model_settings(catalog_runtime(&catalog));
        self.view.runtime_settings.context = context;
        self.view
            .runtime_settings
            .refresh_native_editor(catalog_runtime(&catalog), true);
        self.settings.initial = Some(super::settings_changes::choices(
            &self.view.runtime_settings,
            &catalog,
        ));
        if let Some(change) = self.settings.restore.take() {
            super::settings_changes::restore(&mut self.view.runtime_settings, &catalog, change);
        }
        if refreshing {
            self.view.runtime_settings.model_settings_field = selected;
            self.view.runtime_settings.model_settings_tool_output = mode;
        } else {
            self.view.runtime_settings.model_settings_tool_output = self.view.tool_output.mode;
            self.view.layout.draft = self.view.layout.themes;
            self.view.summary_enabled_draft = catalog.facts.summaries_enabled;
        }
        self.settings.error = None;
        self.settings.keep_draft = false;
        self.settings.catalog = Some(catalog);
    }

    pub(super) fn refresh_settings(&mut self, link: Option<&mut Transport>) -> Result<()> {
        if self.settings.operation.is_some() {
            let uncertain = self
                .settings
                .operation
                .as_ref()
                .and_then(|operation| self.client.operations().get(operation))
                .is_some_and(|record| {
                    matches!(
                        record.state,
                        carlos_proto::submissions::OperationState::Uncertain { .. }
                    )
                });
            if !uncertain {
                self.settings.error =
                    Some("A settings save is pending; wait for its original reply".into());
                return Ok(());
            }
            if let Some(operation) = self.settings.operation.take() {
                self.client.forget_operation(&operation);
            }
            self.settings.submitted = None;
        }
        self.retain_settings_draft();
        let effects = if self.client.settings_panel(&self.session).is_some() {
            self.client.refresh_settings_panel(&self.session)?
        } else {
            self.client.open_settings_panel(&self.session)?
        };
        self.settings.context.refresh();
        self.settings.error = None;
        if let Some(link) = link {
            self.apply_effects(link, effects)?;
            self.drive_settings(link)?;
        }
        Ok(())
    }

    pub(super) fn settings_refreshing(&self) -> bool {
        self.client
            .settings_panel(&self.session)
            .is_some_and(|panel| panel.is_reading())
            || self.settings.context.pending()
    }

    pub(super) fn settings_error(&self) -> Option<&str> {
        self.settings.error.as_deref().or_else(|| {
            self.client
                .settings_panel(&self.session)
                .and_then(|panel| panel.read_error())
        })
    }

    fn retain_settings_draft(&mut self) {
        if let (Some(catalog), Some(initial), Some(replica)) = (
            &self.settings.catalog,
            &self.settings.initial,
            self.client.session(&self.session),
        ) {
            self.settings.restore = Some(super::settings_changes::capture(
                &self.view.runtime_settings,
                catalog,
                initial,
                replica.info().backend,
            ));
        }
    }

    pub(super) fn settings_disconnected(&mut self) {
        self.retain_settings_draft();
        self.settings.context.disconnected();
        if self.view.runtime_settings.show_model_settings {
            self.settings.keep_draft = true;
            self.settings.error = Some("Disconnected; F5 reconnects. Your draft is kept".into());
        }
    }

    pub(super) fn settings_replaced(&mut self) {
        self.settings_disconnected();
        if let Some(operation) = self.settings.operation.take() {
            self.client.forget_operation(&operation);
        }
        self.settings.submitted = None;
        if self.settings.catalog.is_some() {
            self.settings.keep_draft = true;
            self.settings.error =
                Some("Daemon changed; F6 refreshes facts without resending your draft".into());
        }
    }

    pub(super) fn close_settings(&mut self) {
        self.client.close_settings_panel(&self.session);
        self.view.runtime_settings.close_model_settings();
        if self.settings.submitted.is_none() && !self.settings.keep_draft {
            self.settings.catalog = None;
            self.settings.initial = None;
            self.settings.restore = None;
            self.settings.context.edited = false;
            self.settings.error = None;
        }
    }
}

#[cfg(test)]
#[path = "settings_editor_contract_tests.rs"]
mod settings_editor_contract_tests;
