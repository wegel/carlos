//! Settle one settings intent before later ordinary commands and save its captured defaults off-owner.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use carlos_proto::ids::SettingsRevision;
use carlos_proto::settings::SettingsChange;
use carlos_proto::submissions::{ClientOperation, OperationState};

use super::dispatch::rejected;
use super::owner::Owner;
use super::target::Target;
use super::workers::Completion;
use crate::backend::BackendKind;
use crate::session::codex::settings::{self, SettingsUpdate};
use crate::session::runtime_settings::RuntimeDefaults;

pub(super) struct PendingSettings {
    pub(super) operation: ClientOperation,
    pub(super) target: Target,
    pub(super) permission: Arc<AtomicBool>,
    defaults: Option<RuntimeDefaults>,
    accepted: Option<OperationState>,
}

impl Owner {
    pub(super) fn change_settings(
        &mut self,
        operation: ClientOperation,
        expected: SettingsRevision,
        change: SettingsChange,
        save_default: bool,
    ) {
        let prepared = self.session.prepare_settings_change(&expected, change);
        let intent = match prepared {
            Ok(intent)
                if self.client.kind() != BackendKind::Codex
                    || self.session.codex_scope().enabled()
                        && !self.session.codex_scope().closed() =>
            {
                intent
            }
            Ok(_) => {
                self.results.update(
                    &operation,
                    rejected("native Codex settings are not ready; nothing sent"),
                );
                self.notice();
                return;
            }
            Err(error) => {
                self.results.update(&operation, rejected(error.message));
                self.notice();
                return;
            }
        };
        let commands = match self.client.capture_commands() {
            Ok(commands) => commands,
            Err(error) => {
                self.results.update(&operation, rejected(error.to_string()));
                self.notice();
                return;
            }
        };
        self.start_settings(operation, commands, intent, save_default);
    }

    fn start_settings(
        &mut self,
        operation: ClientOperation,
        commands: crate::backend_commands::CommandHandle,
        intent: crate::session::settings_changes::SettingsIntent,
        save_default: bool,
    ) {
        self.settings = Some(PendingSettings {
            operation: operation.clone(),
            target: Target::capture(&self.session, self.client.kind()),
            defaults: save_default.then_some(intent.defaults.clone()),
            accepted: None,
            permission: Arc::new(AtomicBool::new(true)),
        });
        if intent
            .native
            .as_object()
            .is_some_and(|values| values.is_empty())
        {
            self.settings_confirmed(OperationState::Accepted {
                entry: None,
                notice: None,
            });
        } else if self.client.kind() == BackendKind::Claude {
            self.start_backend_settings(commands, intent.native, operation);
        } else if self.client.kind() == BackendKind::Grok {
            self.session.queue_runtime_settings(
                intent.defaults.model,
                intent.defaults.effort,
                intent.defaults.summary,
            );
            self.settings_confirmed(OperationState::Accepted {
                entry: None,
                notice: Some(
                    "Choices saved for the next prompt; the running Grok request was not changed."
                        .into(),
                ),
            });
        } else {
            let scope = self.session.codex_scope();
            let update = settings::save_owned(
                &commands,
                &mut self.session,
                scope,
                intent.native,
                operation,
            );
            self.finish_settings(update);
        }
        self.notice();
    }

    pub(super) fn finish_settings(&mut self, update: SettingsUpdate) {
        let Some((operation, state)) = update.outcome else {
            return;
        };
        let Some(pending) = &self.settings else {
            return;
        };
        if pending.operation != operation || pending.accepted.is_some() {
            return;
        }
        let state =
            if pending.target.matches(&self.session, self.client.kind()) && !self.native.closed {
                state
            } else {
                uncertain("native settings acknowledgement belongs to a retired root")
            };
        self.settings_confirmed(state);
    }

    pub(super) fn settings_confirmed(&mut self, state: OperationState) {
        let pending = self
            .settings
            .as_mut()
            .expect("originating settings request");
        if matches!(state, OperationState::Accepted { .. }) {
            if let Some(defaults) = pending.defaults.take() {
                let operation = pending.operation.clone();
                pending.accepted = Some(state);
                self.workers.ordinary_busy = true;
                self.workers
                    .ordinary
                    .submit(move || Completion::SettingsSaved {
                        operation,
                        error: crate::runtime_preferences::save(&defaults)
                            .err()
                            .map(|error| error.to_string()),
                    });
                return;
            }
        }
        let pending = self.settings.take().expect("settled settings request");
        pending.permission.store(false, Ordering::SeqCst);
        self.results.update(&pending.operation, state);
        self.notice();
    }

    pub(super) fn finish_settings_save(
        &mut self,
        operation: ClientOperation,
        error: Option<String>,
    ) {
        if !self
            .settings
            .as_ref()
            .is_some_and(|pending| pending.operation == operation && pending.accepted.is_some())
        {
            return;
        }
        self.workers.ordinary_busy = false;
        let pending = self.settings.take().expect("confirmed defaults write");
        let mut state = pending.accepted.expect("native result already confirmed");
        if let OperationState::Accepted { notice, .. } = &mut state {
            let saved = error.map_or_else(
                || "Saved defaults for new sessions.".into(),
                |error| {
                    format!("Native settings were accepted, but defaults were not saved: {error}")
                },
            );
            *notice = Some(match notice.take() {
                Some(previous) => format!("{previous} {saved}"),
                None => saved,
            });
        }
        self.results.update(&operation, state);
        self.notice();
    }

    pub(super) fn retire_settings_scope(&mut self) {
        let retire = self.settings.as_ref().is_some_and(|pending| {
            pending.accepted.is_none()
                && (self.native.closed
                    || !pending.target.matches(&self.session, self.client.kind()))
        });
        if retire {
            let pending = self.settings.take().expect("retired settings request");
            pending.permission.store(false, Ordering::SeqCst);
            self.session.set_native_settings_pending(false);
            self.session.retire_owned_settings(&pending.operation);
            self.results.update(&pending.operation, uncertain("native settings connection changed before its outcome was confirmed; no automatic retry"));
            self.notice();
        }
    }
}

fn uncertain(message: &str) -> OperationState {
    OperationState::Uncertain {
        message: message.into(),
        recovery: None,
    }
}
