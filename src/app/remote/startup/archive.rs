//! Keep archive consent separate from private accepted, rejected and uncertain outcomes.

use anyhow::Result;
use carlos_proto::commands::Command;
use carlos_proto::ids::{SavedChoiceId, SavedScopeId};
use carlos_proto::startup::SavedDirection;
use carlos_proto::submissions::OperationState;

use super::picker::Landing;
use super::{Bootstrap, Phase, Submitted};
use crate::app::remote::transport::Transport;

impl Bootstrap {
    pub(super) fn archive(
        &mut self,
        link: &mut Transport,
        scope: SavedScopeId,
        choice: SavedChoiceId,
    ) -> Result<()> {
        let target_scope = scope.clone();
        let target_choice = choice.clone();
        self.submit(
            link,
            move |operation| Command::SavedArchive {
                operation,
                scope: target_scope,
                choice: target_choice,
            },
            |operation| Submitted::Archive {
                operation,
                scope,
                choice,
            },
        )?;
        self.phase = Phase::Archiving;
        Ok(())
    }

    pub(super) fn settle_archive(&mut self, link: &mut Transport) -> Result<()> {
        let Some(Submitted::Archive {
            operation,
            scope,
            choice,
        }) = &self.submitted
        else {
            return Ok(());
        };
        let Some(result) = self.client.operations().get(operation) else {
            return Ok(());
        };
        let operation = operation.clone();
        let scope = scope.clone();
        let choice = choice.clone();
        match &result.state {
            OperationState::Accepted { .. } => {
                self.client.forget_operation(&operation);
                self.submitted = None;
                self.archived(link, scope, choice)?;
            }
            OperationState::Rejected { message, .. } => {
                self.error = Some(format!("Archive rejected: {message}"));
                self.client.forget_operation(&operation);
                self.submitted = None;
                if let Some(picker) = &mut self.picker {
                    picker.confirm = false;
                }
                self.phase = Phase::Picking;
            }
            OperationState::Uncertain { message, .. } => {
                self.error = Some(format!(
                    "Archive could not be confirmed: {message}. Press r for fresh choices"
                ));
                if let Some(picker) = &mut self.picker {
                    picker.confirm = false;
                    picker.stale = true;
                }
                self.phase = Phase::Picking;
            }
            _ => {}
        }
        Ok(())
    }

    fn archived(
        &mut self,
        link: &mut Transport,
        scope: SavedScopeId,
        choice: SavedChoiceId,
    ) -> Result<()> {
        self.phase = Phase::Picking;
        self.error = Some("Archived session".into());
        let Some(picker) = self
            .picker
            .as_mut()
            .filter(|picker| picker.page.scope == scope)
        else {
            return self.list(link);
        };
        if picker.stale {
            return self.list(link);
        }
        picker.page.rows.retain(|row| row.choice != choice);
        picker.selected = picker
            .selected
            .min(picker.page.rows.len().saturating_sub(1));
        picker.confirm = false;
        if !picker.page.rows.is_empty() {
            return Ok(());
        }
        if picker.page.has_older {
            self.page(link, SavedDirection::Older, Landing::First)?;
        } else if picker.page.has_newer {
            self.page(link, SavedDirection::Newer, Landing::Last)?;
        } else {
            self.phase = Phase::Empty;
        }
        Ok(())
    }
}
