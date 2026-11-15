//! Retain only accepted shared settings; private editors stay with each platform.

use carlos_proto::settings::{RuntimeChoices, SettingsFacts};

use crate::{ChangeSet, Replica};

impl Replica {
    /// Borrow the backend's accepted choices and current settings-read precondition.
    pub fn settings(&self) -> Option<&SettingsFacts> {
        self.settings.as_ref()
    }

    pub(crate) fn replace_settings(
        &mut self,
        settings: Option<SettingsFacts>,
        changed: &mut ChangeSet,
    ) {
        changed.settings = self.settings != settings;
        self.settings = settings;
    }

    pub(crate) fn settings_bytes(&self) -> usize {
        self.settings.as_ref().map_or(0, |facts| {
            choices_bytes(&facts.current) + choices_bytes(&facts.pending) + facts.label.len() + 1024
        })
    }
}

fn choices_bytes(choices: &RuntimeChoices) -> usize {
    [&choices.model, &choices.effort, &choices.summary]
        .iter()
        .map(|value| value.as_ref().map_or(0, String::len))
        .sum()
}
