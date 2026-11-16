//! Collect one complete model list before transferring it to a platform's private settings editor.

use carlos_proto::commands::RequestId;
use carlos_proto::settings::{ModelInfo, SettingsFacts, SettingsPage};

/// Freeze the complete checked model list and the native choices from which an editor opened.
pub struct SettingsCatalog {
    pub facts: SettingsFacts,
    pub models: Vec<ModelInfo>,
}

/// Read status and a completed model list; private edits and save decisions stay on the platform.
#[derive(Default)]
pub struct SettingsPanel {
    pub(crate) read: Option<RequestId>,
    pub(crate) fetch: Option<Fetch>,
    pub(crate) ready: Option<Ready>,
    pub(crate) error: Option<String>,
    pub(crate) dirty: bool,
    pub(crate) delivered: bool,
    bytes: usize,
}

#[derive(Default)]
pub(crate) struct Fetch {
    pub facts: Option<SettingsFacts>,
    pub models: Vec<ModelInfo>,
    pub bytes: usize,
    pub next: u32,
    pub waiting: Option<SettingsPage>,
}

pub(crate) struct Ready {
    pub catalog: SettingsCatalog,
    pub bytes: usize,
}

impl SettingsPanel {
    /// Borrow the complete list before an editor takes ownership of it.
    pub fn catalog(&self) -> Option<&SettingsCatalog> {
        self.ready.as_ref().map(|ready| &ready.catalog)
    }

    /// Distinguish an unfinished list from a completed list or an explicit read failure.
    pub fn is_reading(&self) -> bool {
        self.fetch.is_some() || self.read.is_some()
    }

    /// Borrow this reader's error without replacing a platform's private save error.
    pub fn read_error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.bytes
    }

    pub(crate) fn recount(&mut self) {
        self.bytes = 256
            + self.error.as_ref().map_or(0, String::len)
            + self.ready.as_ref().map_or(0, |ready| ready.bytes)
            + self.fetch.as_ref().map_or(0, |fetch| {
                fetch.bytes
                    + fetch.facts.as_ref().map_or(0, facts_bytes)
                    + fetch.waiting.as_ref().map_or(0, |page| {
                        model_bytes(&page.models) + facts_bytes(&page.settings)
                    })
            });
    }

    pub(crate) fn restart(&mut self) -> Option<RequestId> {
        self.fetch = Some(Fetch::default());
        self.ready = None;
        self.error = None;
        self.dirty = true;
        self.recount();
        self.read.take()
    }

    pub(crate) fn disconnected(&mut self) {
        self.read = None;
        self.fetch = (!self.delivered && self.ready.is_none()).then(Fetch::default);
        self.dirty = true;
        self.recount();
    }
}

pub(crate) fn model_bytes(models: &[ModelInfo]) -> usize {
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

pub(crate) fn facts_bytes(facts: &SettingsFacts) -> usize {
    [&facts.current, &facts.pending]
        .iter()
        .map(|choices| {
            choices.model.as_ref().map_or(0, String::len)
                + choices.effort.as_ref().map_or(0, String::len)
                + choices.summary.as_ref().map_or(0, String::len)
        })
        .sum::<usize>()
        + facts.label.len()
        + 256
}
