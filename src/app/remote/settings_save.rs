//! Validate one private Save before sending its distinct host and native choices.

use anyhow::Result;

use super::settings_appearance::Appearance;
use super::settings_changes;
use super::settings_intent::{LabelIntent, NativeIntent, SaveIntent};
use super::transport::Transport;
use super::Device;

impl Device {
    pub(super) fn save_settings(&mut self, link: Option<&mut Transport>) -> Result<()> {
        if self.settings.submitted.is_some() {
            self.settings.error =
                Some("A settings save is still pending; your draft is kept".into());
            return Ok(());
        }
        if self.settings_refreshing() {
            self.settings.error =
                Some("Reading current settings; your edited fields stay private".into());
            return Ok(());
        }
        let Some(link) =
            link.filter(|link| self.client.session_is_ready(link.connection, &self.session))
        else {
            self.settings.error = Some(
                "Disconnected or restoring; F5 reconnects. Your choices have not been sent".into(),
            );
            return Ok(());
        };
        match self.capture_settings_save() {
            Ok(intent) => self.settings.submitted = Some(intent),
            Err(error) => {
                self.settings.error = Some(error.to_string());
                return Ok(());
            }
        }
        if self.settings.appearance_save.is_some() {
            self.settings.error =
                Some("Finishing the previous save; these choices are queued".into());
        }
        self.drive_settings_save(link)?;
        Ok(())
    }

    fn capture_settings_save(&self) -> Result<SaveIntent> {
        let catalog = self
            .settings
            .catalog
            .as_ref()
            .expect("loaded settings catalog");
        let backend = self
            .client
            .session(&self.session)
            .expect("ready session")
            .info()
            .backend;
        let change = settings_changes::change(
            &self.view.runtime_settings,
            catalog,
            self.settings
                .initial
                .as_ref()
                .expect("initial displayed choices"),
            backend,
        )?;
        Ok(SaveIntent {
            context: self.settings_context_choice()?,
            native: Some(NativeIntent {
                expected: catalog.facts.revision.clone(),
                change,
            }),
            labels: (self.view.summary_enabled_draft != catalog.facts.summaries_enabled).then_some(
                LabelIntent {
                    original: catalog.facts.summaries_enabled,
                    enabled: self.view.summary_enabled_draft,
                },
            ),
            appearance: Appearance::capture(&self.view),
            step: None,
            error: None,
            notice: None,
        })
    }

    pub(super) fn finish_settings_save(&mut self) {
        let intent = self
            .settings
            .submitted
            .take()
            .expect("finished settings intent");
        self.view.tool_output.mode = intent.appearance.mode;
        self.view.layout.themes = intent.appearance.themes;
        self.settings.appearance_save = Some(intent.appearance.save());
        self.view.render_cache.invalidate_all();
        self.dirty_from(0);
        self.view.status_hint = match (&intent.error, intent.notice) {
            (Some(error), Some(notice)) => Some(format!("{error}; {notice}")),
            (Some(error), None) => Some(error.clone()),
            (None, notice) => notice,
        };
        self.settings.error = intent.error;
        self.settings.keep_draft = self.settings.error.is_some();
        if self.settings.error.is_none() {
            let _ = self.client.close_settings_panel(&self.session);
            let _ = self.client.close_context_panel(&self.session);
            self.view.runtime_settings.close_model_settings();
            self.settings.catalog = None;
            self.settings.initial = None;
        }
    }

    pub(super) fn poll_settings_appearance(&mut self) -> bool {
        let Some(result) = self
            .settings
            .appearance_save
            .as_ref()
            .and_then(|save| save.poll())
        else {
            return false;
        };
        self.settings.appearance_save = None;
        if let Err(error) = result {
            self.view.status_hint = Some(format!(
                "Settings applied; could not save device appearance: {error}"
            ));
        }
        true
    }
}
