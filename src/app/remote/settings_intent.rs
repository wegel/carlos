//! Capture one explicit Save without rebasing or replaying its native model choices.

use carlos_proto::commands::Command;
use carlos_proto::context_defaults::ContextChoice;
use carlos_proto::ids::{SessionId, SettingsRevision};
use carlos_proto::setting_commands::{SetContext, SetSettings, SetToolLabels};
use carlos_proto::settings::{SettingsChange, SettingsFacts};
use carlos_proto::submissions::ClientOperation;

use super::settings_appearance::Appearance;

pub(super) struct NativeIntent {
    pub expected: SettingsRevision,
    pub change: SettingsChange,
}

pub(super) struct LabelIntent {
    pub original: bool,
    pub enabled: bool,
}

#[derive(Clone, Copy)]
pub(super) enum Step {
    Context(ContextChoice),
    Native,
    Labels,
}

pub(super) enum Write {
    Context(ContextChoice),
    Native(NativeIntent),
    Labels {
        expected: SettingsRevision,
        enabled: bool,
    },
}

pub(super) struct SaveIntent {
    pub context: Option<ContextChoice>,
    pub native: Option<NativeIntent>,
    pub labels: Option<LabelIntent>,
    pub appearance: Appearance,
    pub step: Option<Step>,
    pub error: Option<String>,
    pub notice: Option<String>,
}

impl SaveIntent {
    pub fn take_next(&mut self, facts: &SettingsFacts) -> Option<Write> {
        if let Some(choice) = self.context.take() {
            self.step = Some(Step::Context(choice));
            return Some(Write::Context(choice));
        }
        if let Some(native) = self.native.take() {
            self.step = Some(Step::Native);
            return Some(Write::Native(native));
        }
        let labels = self.labels.take()?;
        // Only the independent boolean may follow a newer model revision; native choices never do.
        if facts.summaries_enabled != labels.original {
            self.error.get_or_insert_with(|| {
                "AI-label setting changed; F6 refreshes facts before another save".into()
            });
            return None;
        }
        self.step = Some(Step::Labels);
        Some(Write::Labels {
            expected: facts.revision.clone(),
            enabled: labels.enabled,
        })
    }

    pub fn notice(&mut self, notice: Option<String>) {
        if let Some(notice) = notice {
            match &mut self.notice {
                Some(previous) => {
                    previous.push_str("; ");
                    previous.push_str(&notice);
                }
                previous => *previous = Some(notice),
            }
        }
    }
}

impl Write {
    pub fn command(self, session: SessionId, operation: ClientOperation) -> Command {
        match self {
            Self::Context(choice) => Command::ContextSet {
                session,
                request: SetContext { operation, choice },
            },
            Self::Native(native) => Command::SettingsSet {
                session,
                request: SetSettings {
                    operation,
                    expected: native.expected,
                    change: native.change,
                    save_default: true,
                },
            },
            Self::Labels { expected, enabled } => Command::ToolLabelsSet {
                session,
                request: SetToolLabels {
                    operation,
                    expected,
                    enabled,
                    save_default: true,
                },
            },
        }
    }
}
