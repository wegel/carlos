//! Load one checked model catalog for this terminal's private settings editor.

use anyhow::Result;
use carlos_client::{Effects, ReadReply};
use carlos_core::session::runtime_settings::RuntimeDefaults;
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::settings::SettingsChange;
use carlos_proto::settings::{ModelInfo, SettingsFacts, SettingsPage};
use carlos_proto::submissions::ClientOperation;

use super::transport::Transport;
use super::Device;
use crate::app::runtime_settings_state::RuntimeSettingsRef;

pub(super) struct Catalog {
    pub facts: SettingsFacts,
    pub models: Vec<ModelInfo>,
}

impl Catalog {
    pub fn runtime(&self) -> RuntimeSettingsRef<'_> {
        RuntimeSettingsRef::from((&self.facts, self.models.as_slice()))
    }
}

#[derive(Default)]
pub(super) struct Fetch {
    facts: Option<SettingsFacts>,
    models: Vec<ModelInfo>,
    bytes: usize,
    next: u32,
    waiting: Option<SettingsPage>,
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
    restore: Option<SettingsChange>,
    read: Option<RequestId>,
    fetch: Option<Fetch>,
}

impl Device {
    pub(super) fn open_settings(&mut self, link: Option<&mut Transport>) -> Result<()> {
        self.view.runtime_settings.show_model_settings = true;
        if self.settings.submitted.is_some()
            || self.settings.catalog.is_some() && self.settings.error.is_some()
        {
            return Ok(());
        }
        let appearance_save = self.settings.appearance_save.take();
        let read = self.settings.read.take();
        let mut context = std::mem::take(&mut self.settings.context);
        context.reopen();
        self.view.runtime_settings.context = Default::default();
        self.settings = Settings {
            fetch: Some(Fetch::default()),
            appearance_save,
            read,
            context,
            ..Default::default()
        };
        if let Some(link) = link {
            self.drive_settings(link)?;
        } else {
            self.settings.error = Some("Disconnected; F5 reconnects. Your draft is kept".into());
        }
        Ok(())
    }

    pub(super) fn apply_settings_reads(&mut self, effects: &mut Effects) -> bool {
        let mut changed = self.apply_context_reads(effects);
        for result in std::mem::take(&mut effects.replies) {
            if self.settings.read.is_some() && result.id == self.settings.read {
                self.settings.read = None;
                if let ReadReply::Settings(page) = result.reply {
                    if let Some(fetch) = &mut self.settings.fetch {
                        fetch.waiting = Some(page);
                    }
                    changed = true;
                    continue;
                }
            }
            effects.replies.push(result);
        }
        for failure in &effects.failures {
            if self.settings.read.is_some() && failure.id == self.settings.read {
                self.settings.read = None;
                if self.settings_fetch_changed() {
                    self.settings.fetch = Some(Fetch::default());
                } else {
                    self.settings.fetch = None;
                    self.settings.error = Some(failure.error.message.clone());
                }
                changed = true;
            }
        }
        changed
    }

    /// A page may arrive before its numbered settings fact; hold one page until that cursor arrives.
    pub(super) fn drive_settings(&mut self, link: &mut Transport) -> Result<bool> {
        let mut changed = self.drive_settings_save(link)?;
        if !self.view.runtime_settings.show_model_settings {
            return Ok(false);
        }
        self.drive_settings_context(link)?;
        if self.settings_fetch_changed() {
            self.settings.fetch = Some(Fetch::default());
            changed = true;
        }
        changed |= self.settle_settings_page();
        let Some(fetch) = &self.settings.fetch else {
            return Ok(changed);
        };
        if self.settings.read.is_none()
            && fetch.waiting.is_none()
            && self.client.session_is_ready(link.connection, &self.session)
        {
            let outgoing = self.client.read(
                link.connection,
                Command::SettingsRead {
                    session: self.session.clone(),
                    expected: fetch.facts.as_ref().map(|facts| facts.revision.clone()),
                    offset: fetch.next,
                    limit: Some(32),
                },
            )?;
            self.settings.read = Some(outgoing.id());
            link.send(outgoing)?;
        }
        Ok(changed)
    }

    fn settings_fetch_changed(&self) -> bool {
        let current = self
            .client
            .session(&self.session)
            .and_then(|replica| replica.settings());
        self.settings
            .fetch
            .as_ref()
            .and_then(|fetch| fetch.facts.as_ref())
            .zip(current)
            .is_some_and(|(old, current)| old.revision != current.revision)
    }

    fn settle_settings_page(&mut self) -> bool {
        let Some(fetch) = &mut self.settings.fetch else {
            return false;
        };
        let Some(page) = &fetch.waiting else {
            return false;
        };
        let Some(replica) = self.client.session(&self.session) else {
            return false;
        };
        if replica.sequence() < page.seq {
            return false;
        }
        if replica
            .settings()
            .is_none_or(|facts| facts.revision != page.settings.revision)
        {
            self.settings.fetch = Some(Fetch::default());
            return true;
        }
        let page = fetch.waiting.take().expect("checked settings page");
        if page.offset != fetch.next {
            return true;
        }
        fetch.bytes = fetch.bytes.saturating_add(model_bytes(&page.models));
        if fetch.bytes > 8 * 1024 * 1024 {
            self.settings.fetch = None;
            self.settings.error = Some("Model catalog exceeds the eight MiB editor budget".into());
            return true;
        }
        fetch.models.extend(page.models);
        fetch.facts = Some(page.settings);
        if let Some(next) = page.next {
            fetch.next = next;
        } else {
            let fetch = self.settings.fetch.take().expect("complete model catalog");
            let catalog = Catalog {
                facts: fetch.facts.expect("catalog facts"),
                models: fetch.models,
            };
            self.install_settings_catalog(catalog);
        }
        true
    }
    fn install_settings_catalog(&mut self, catalog: Catalog) {
        let refreshing = self.settings.catalog.is_some();
        let selected = self.view.runtime_settings.model_settings_field;
        let mode = self.view.runtime_settings.model_settings_tool_output;
        let context = std::mem::take(&mut self.view.runtime_settings.context);
        self.view
            .runtime_settings
            .open_model_settings(catalog.runtime());
        self.view.runtime_settings.context = context;
        self.view
            .runtime_settings
            .refresh_native_editor(catalog.runtime(), true);
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
        self.settings.fetch = Some(Fetch::default());
        self.settings.context.refresh();
        self.settings.error = None;
        if let Some(link) = link {
            self.drive_settings(link)?;
        }
        Ok(())
    }

    pub(super) fn settings_refreshing(&self) -> bool {
        self.settings.fetch.is_some()
            || self.settings.read.is_some()
            || self.settings.context.pending()
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
        self.settings.read = None;
        self.settings.fetch = self.settings.catalog.is_none().then(Fetch::default);
        if self.view.runtime_settings.show_model_settings {
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
            self.settings.error =
                Some("Daemon changed; F6 refreshes facts without resending your draft".into());
        }
    }

    pub(super) fn close_settings(&mut self) {
        self.view.runtime_settings.close_model_settings();
        if self.settings.operation.is_none() && self.settings.error.is_none() {
            self.settings.catalog = None;
            self.settings.initial = None;
            self.settings.restore = None;
            self.settings.fetch = None;
            self.settings.context.edited = false;
        }
    }
}

fn model_bytes(models: &[ModelInfo]) -> usize {
    models
        .iter()
        .map(|model| {
            model.model.len()
                + model.display_name.len()
                + model.default_effort.as_ref().map_or(0, String::len)
                + model
                    .supported_efforts
                    .iter()
                    .map(|effort| effort.len() + 24)
                    .sum::<usize>()
                + 128
        })
        .sum()
}
