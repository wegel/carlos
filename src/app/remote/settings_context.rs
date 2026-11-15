//! Read the daemon host's next-launch size while keeping this terminal's draft private.

use anyhow::{bail, Result};
use carlos_client::{Effects, ReadReply};
use carlos_core::backend::BackendKind;
use carlos_core::context_preferences;
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::context_defaults::{ContextChoice, ContextDefaults, ContextPolicy};
use carlos_proto::session::Backend;

use super::transport::Transport;
use super::Device;
use crate::app::context_settings::ContextSettings;

#[derive(Default)]
pub(super) struct Context {
    read: Option<RequestId>,
    waiting: bool,
    defaults: Option<ContextDefaults>,
    error: Option<String>,
    pub edited: bool,
}

impl Context {
    pub fn reopen(&mut self) {
        *self = Self {
            read: self.read.take(),
            waiting: true,
            ..Default::default()
        };
    }

    pub fn refresh(&mut self) {
        self.waiting = true;
    }

    pub fn disconnected(&mut self) {
        self.read = None;
        self.waiting = true;
    }

    pub fn pending(&self) -> bool {
        self.waiting || self.read.is_some()
    }
}

impl Device {
    pub(super) fn drive_settings_context(&mut self, link: &mut Transport) -> Result<()> {
        let context = &mut self.settings.context;
        if context.waiting
            && context.read.is_none()
            && self.client.session_is_ready(link.connection, &self.session)
        {
            let request = self.client.read(
                link.connection,
                Command::ContextRead {
                    session: self.session.clone(),
                },
            )?;
            context.read = Some(request.id());
            context.waiting = false;
            link.send(request)?;
        }
        Ok(())
    }

    pub(super) fn apply_context_reads(&mut self, effects: &mut Effects) -> bool {
        let mut changed = false;
        for reply in std::mem::take(&mut effects.replies) {
            if self.settings.context.read.is_some() && reply.id == self.settings.context.read {
                self.settings.context.read = None;
                if let ReadReply::ContextDefaults(defaults) = reply.reply {
                    self.settings.context.defaults = Some(defaults);
                    self.settings.context.error = None;
                    self.install_context_editor();
                    changed = true;
                    continue;
                }
            }
            effects.replies.push(reply);
        }
        for failure in &effects.failures {
            if self.settings.context.read.is_some() && failure.id == self.settings.context.read {
                self.settings.context.read = None;
                self.settings.context.defaults = None;
                self.settings.context.error = Some(failure.error.message.clone());
                self.install_context_editor();
                changed = true;
            }
        }
        changed
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
        "Next backend launch applies size; current session stays unchanged"
    }
}
