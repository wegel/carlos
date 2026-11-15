//! Apply the shared label preference and drain the existing worker on its session owner.

use std::sync::mpsc;

use carlos_proto::ids::SettingsRevision;
use carlos_proto::submissions::{ClientOperation, OperationState};

use super::dispatch::rejected;
use super::owner::Owner;
use super::workers::Completion;
use crate::ui_preferences;

impl Owner {
    pub(super) fn start_tool_labels(&mut self) {
        let (sender, receiver) = mpsc::sync_channel(128);
        self.session
            .configure_summary_worker_waking(sender, self.channels.wake.clone());
        self.tool_labels = Some(receiver);
    }

    pub(super) fn drain_tool_labels(&mut self) {
        for _ in 0..64 {
            let Some(event) = self
                .tool_labels
                .as_ref()
                .and_then(|receiver| receiver.try_recv().ok())
            else {
                return;
            };
            self.session.handle_summary_event(event);
            self.notice();
        }
        // A full batch may leave ready results behind after consuming the coalesced wake.
        let _ = self.channels.wake.try_send(());
    }

    pub(super) fn change_tool_labels(
        &mut self,
        operation: ClientOperation,
        expected: SettingsRevision,
        enabled: bool,
        save_default: bool,
    ) {
        if expected != self.session.settings_revision() {
            self.results.update(
                &operation,
                rejected("settings changed; refresh before changing AI tool labels"),
            );
        } else {
            self.session.apply_summary_setting(enabled);
            if save_default {
                self.tool_labels_save = Some(operation.clone());
                self.workers.ordinary_busy = true;
                self.workers
                    .ordinary
                    .submit(move || Completion::ToolLabelsSaved {
                        operation,
                        error: ui_preferences::save_summary_enabled(enabled)
                            .err()
                            .map(|error| format!("{error:#}")),
                    });
            } else {
                self.results.update(&operation, accepted(None));
            }
        }
        self.notice();
    }

    pub(super) fn finish_tool_labels_save(
        &mut self,
        operation: ClientOperation,
        error: Option<String>,
    ) {
        if self.tool_labels_save.as_ref() != Some(&operation) {
            return;
        }
        self.tool_labels_save = None;
        self.workers.ordinary_busy = false;
        let notice = match error {
            None => {
                "AI tool labels changed; saved the default for new and restored sessions.".into()
            }
            Some(error) => {
                let detail: String = error.chars().take(1024).collect();
                format!("AI tool labels changed, but their default was not saved: {detail}")
            }
        };
        self.results.update(&operation, accepted(Some(notice)));
        self.notice();
    }
}

fn accepted(notice: Option<String>) -> OperationState {
    OperationState::Accepted {
        entry: None,
        notice,
    }
}
