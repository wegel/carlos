//! Read the daemon host's next-launch size while keeping this terminal's draft private.

use anyhow::{bail, Result};
use carlos_client::panels::PanelKind;
use carlos_client::Effects;
use carlos_core::backend::BackendKind;
use carlos_core::context_preferences;
use carlos_proto::context_defaults::{ContextChoice, ContextDefaults, ContextPolicy};
use carlos_proto::session::Backend;

use super::transport::Transport;
use super::Device;
use crate::app::context_settings::ContextSettings;

/// Keep only the editor's saved-choice baseline, errors and explicit text-edit flag.
#[derive(Default)]
pub(super) struct Context {
    defaults: Option<ContextDefaults>,
    error: Option<String>,
    pub edited: bool,
}

impl Device {
    pub(super) fn open_settings_context(&mut self, link: Option<&mut Transport>) -> Result<()> {
        let effects = self.client.open_context_panel(&self.session)?;
        if let Some(link) = link {
            self.apply_effects(link, effects)?;
        }
        Ok(())
    }

    pub(super) fn refresh_settings_context(&mut self, link: Option<&mut Transport>) -> Result<()> {
        let effects = if self.client.context_panel(&self.session).is_some() {
            self.client.refresh_context_panel(&self.session)?
        } else {
            self.client.open_context_panel(&self.session)?
        };
        if let Some(link) = link {
            self.apply_effects(link, effects)?;
        }
        Ok(())
    }

    pub(super) fn apply_context_reads(&mut self, effects: &Effects) -> bool {
        if !effects
            .panels
            .iter()
            .any(|change| change.session == self.session && change.kind == PanelKind::Context)
        {
            return false;
        }
        let Some(result) = self.client.take_context_defaults(&self.session) else {
            return false;
        };
        match result {
            Ok(defaults) => {
                self.settings.context.defaults = Some(defaults);
                self.settings.context.error = None;
            }
            Err(error) => {
                self.settings.context.defaults = None;
                self.settings.context.error = Some(error);
            }
        }
        self.install_context_editor();
        true
    }

    pub(super) fn install_context_editor(&mut self) {
        let context = &self.settings.context;
        let editor = &mut self.view.runtime_settings.context;
        let draft = context.edited.then(|| editor.draft.clone());
        let policy = context.defaults.as_ref().map(|defaults| defaults.policy);
        let backend =
            self.client
                .session(&self.session)
                .map(|replica| match replica.info().backend {
                    Backend::Codex => BackendKind::Codex,
                    Backend::Claude => BackendKind::Claude,
                    Backend::Grok => BackendKind::Grok,
                });
        let supported =
            policy != Some(ContextPolicy::Unsupported) && backend != Some(BackendKind::Grok);
        *editor = ContextSettings::from_saved(
            backend.filter(|_| supported),
            policy == Some(ContextPolicy::AttachedCodex),
            context
                .defaults
                .as_ref()
                .and_then(|defaults| defaults.saved_tokens),
        );
        if let Some(draft) = draft {
            editor.draft = draft;
        } else if context.defaults.is_none() {
            editor.draft.clear();
        }
        editor.error.clone_from(&context.error);
    }

    pub(super) fn settings_context_choice(&self) -> Result<Option<ContextChoice>> {
        let context = &self.settings.context;
        let editor = &self.view.runtime_settings.context;
        if editor.backend.is_none() || editor.attached || !context.edited {
            return Ok(None);
        }
        let tokens = context_preferences::parse(&editor.draft)?;
        if context
            .defaults
            .as_ref()
            .is_some_and(|saved| saved.saved_tokens == tokens)
        {
            return Ok(None);
        }
        if tokens.is_some()
            && context
                .defaults
                .as_ref()
                .is_some_and(|saved| saved.policy == ContextPolicy::ClaudeShellOverride)
        {
            bail!("Unset CLAUDE_CODE_AUTO_COMPACT_WINDOW before choosing a size");
        }
        Ok(Some(ContextChoice::from(tokens)))
    }

    pub(super) fn context_saved(&mut self, choice: ContextChoice) {
        if let Some(defaults) = &mut self.settings.context.defaults {
            defaults.saved_tokens = choice.into();
        }
        self.settings.context.edited = false;
    }

    pub(super) fn settings_context_help(&self) -> &str {
        let editor = &self.view.runtime_settings.context;
        if let Some(error) = editor.error.as_deref() {
            return error;
        }
        if editor.attached {
            return "Attached Codex server owns the size";
        }
        if self
            .settings
            .context
            .defaults
            .as_ref()
            .is_some_and(|saved| saved.policy == ContextPolicy::ClaudeShellOverride)
        {
            return "Daemon shell sets size; choose Auto";
        }
        "Start a new session to apply size"
    }
}
