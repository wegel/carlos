//! Lend settings values to the editor without creating or mutating a core session.

use carlos_proto::settings::{ModelInfo, SettingsFacts};

#[derive(Clone, Copy)]
pub(in crate::app) struct ChoiceRefs<'a> {
    pub model: &'a Option<String>,
    pub effort: &'a Option<String>,
    pub summary: &'a Option<String>,
}

/// The same editor borrows either embedded facts or an accepted client catalog.
#[derive(Clone, Copy)]
pub(in crate::app) struct RuntimeSettingsRef<'a> {
    pub current: ChoiceRefs<'a>,
    pub pending: ChoiceRefs<'a>,
    pub models: &'a [ModelInfo],
    pub supports_effort: bool,
    pub supports_summary: bool,
}

impl<'a> From<(&'a SettingsFacts, &'a [ModelInfo])> for RuntimeSettingsRef<'a> {
    fn from((settings, models): (&'a SettingsFacts, &'a [ModelInfo])) -> Self {
        Self {
            current: ChoiceRefs {
                model: &settings.current.model,
                effort: &settings.current.effort,
                summary: &settings.current.summary,
            },
            pending: ChoiceRefs {
                model: &settings.pending.model,
                effort: &settings.pending.effort,
                summary: &settings.pending.summary,
            },
            models,
            supports_effort: settings.supports_effort,
            supports_summary: settings.supports_summary,
        }
    }
}
