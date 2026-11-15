//! Wait for Claude settings on the ordinary worker and settle only its original private operation.

use carlos_proto::submissions::{ClientOperation, OperationState};
use serde_json::Value;

use super::dispatch::rejected;
use super::owner::Owner;
use super::workers::Completion;
use crate::backend_commands::{CommandHandle, SettingsFailure, SettingsPatch, SettingsResult};
use crate::dispatch_guard::DispatchGuard;

impl Owner {
    pub(super) fn start_backend_settings(
        &mut self,
        commands: CommandHandle,
        native: Value,
        operation: ClientOperation,
    ) {
        let pending = self.settings.as_ref().expect("prepared Claude settings");
        let permission = DispatchGuard::from_flag(pending.permission.clone());
        let commands = match commands.guarded(permission) {
            Ok(commands) => commands,
            Err(error) => {
                self.settings_confirmed(rejected(error.to_string()));
                return;
            }
        };
        let patch = SettingsPatch {
            model: native
                .get("model")
                .map(|value| value.as_str().map(str::to_owned)),
            effort: native
                .get("effort")
                .map(|value| value.as_str().map(str::to_owned)),
        };
        self.session.set_native_settings_pending(true);
        self.workers.ordinary_busy = true;
        self.workers
            .ordinary
            .submit(move || Completion::BackendSettings {
                operation,
                result: commands.change_settings(patch),
            });
    }

    pub(super) fn finish_backend_settings(
        &mut self,
        operation: ClientOperation,
        result: SettingsResult,
    ) {
        self.workers.ordinary_busy = false;
        let Some(pending) = self
            .settings
            .as_ref()
            .filter(|pending| pending.operation == operation)
        else {
            return;
        };
        let state = if pending.target.matches(&self.session, self.client.kind())
            && !self.native.closed
        {
            self.session.set_native_settings_pending(false);
            let fields = confirmed_fields(&result.accepted);
            self.session.accept_claude_settings(result.accepted);
            match result.failure {
                None => OperationState::Accepted {
                    entry: None,
                    notice: None,
                },
                Some(SettingsFailure::Rejected(message)) => {
                    rejected(format!("{fields}{message}; defaults were not saved"))
                }
                Some(SettingsFailure::Uncertain(message)) => uncertain(format!(
                    "{fields}{message}; defaults were not saved; no automatic retry"
                )),
            }
        } else {
            uncertain("Claude settings reply belongs to a retired helper or root; defaults were not saved; no automatic retry".into())
        };
        self.settings_confirmed(state);
    }
}

fn confirmed_fields(patch: &SettingsPatch) -> String {
    match (patch.model.is_some(), patch.effort.is_some()) {
        (true, true) => "Claude confirmed model and effort. ".into(),
        (true, false) => "Claude confirmed model. ".into(),
        (false, true) => "Claude confirmed effort. ".into(),
        (false, false) => String::new(),
    }
}

fn uncertain(message: String) -> OperationState {
    OperationState::Uncertain {
        message,
        recovery: None,
    }
}
