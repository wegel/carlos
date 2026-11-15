//! Change Claude fields one at a time and retain earlier acknowledgements after a later refusal.

use std::time::Duration;

use anyhow::{anyhow, Result};
use serde_json::json;

use super::Commands;
use crate::backend_commands::{SettingsFailure, SettingsPatch, SettingsResult};
use crate::claude_backend::transport::ControllerFailure;
use crate::dispatch_guard::DispatchRetired;

impl Commands {
    pub(super) fn change_checked_settings(&self, patch: SettingsPatch) -> SettingsResult {
        let mut result = SettingsResult::default();
        for (name, choice, accepted) in [
            ("model", patch.model, &mut result.accepted.model),
            ("effort", patch.effort, &mut result.accepted.effort),
        ] {
            let Some(choice) = choice else { continue };
            let choice = choice.filter(|value| name != "effort" || value != "auto");
            match self.change_field(name, choice.clone()) {
                Ok(()) => *accepted = Some(choice),
                Err(error) => {
                    let detail: String = error.to_string().chars().take(1024).collect();
                    let message = format!("Claude {name} change failed: {detail}");
                    result.failure = Some(
                        if error.is::<DispatchRetired>()
                            || matches!(
                                error.downcast_ref::<ControllerFailure>(),
                                Some(ControllerFailure::Rejected(_))
                            )
                        {
                            SettingsFailure::Rejected(message)
                        } else {
                            SettingsFailure::Uncertain(message)
                        },
                    );
                    break;
                }
            }
        }
        result
    }

    fn change_field(&self, name: &str, choice: Option<String>) -> Result<()> {
        self.helper_call(
            if name == "model" {
                "model.set"
            } else {
                "effort.set"
            },
            if name == "model" {
                json!({"model":choice})
            } else {
                json!({"effort":choice})
            },
            Duration::from_secs(10),
        )?;
        self.check()?;
        let mut settings = self
            .runtime_settings
            .lock()
            .map_err(|_| anyhow!("Claude settings lock poisoned"))?;
        if name == "model" {
            settings.model = choice;
        } else {
            settings.effort = choice;
        }
        Ok(())
    }
}
