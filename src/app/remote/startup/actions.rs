//! Submit each explicit startup choice once and keep its private operation across transport loss.

use anyhow::Result;
use carlos_proto::commands::Command;
use carlos_proto::session::Backend;
use carlos_proto::startup::{OpenTarget, SavedDirection, SavedPage, SavedPurpose};
use carlos_proto::submissions::ClientOperation;

use super::options::Intent;
use super::picker::{Landing, Picker};
use super::{Bootstrap, Phase, Submitted};
use crate::app::remote::transport::Transport;

impl Bootstrap {
    pub(super) fn start(&mut self, link: &mut Transport) -> Result<()> {
        match &self.options.intent {
            Intent::New => self.create(link),
            Intent::Resume(address) => self.open(
                link,
                OpenTarget::Resume {
                    address: address.clone(),
                },
            ),
            Intent::Picker | Intent::Continue => self.list(link),
        }
    }

    fn create(&mut self, link: &mut Transport) -> Result<()> {
        let backend = self.options.backend;
        let cwd = self.options.cwd.clone();
        let launch = self.options.launch.clone();
        let tool_labels = self.options.tool_labels;
        self.submit(
            link,
            move |operation| Command::Create {
                operation,
                backend,
                cwd,
                launch,
                tool_labels,
                name: None,
            },
            Submitted::Open,
        )?;
        self.phase = Phase::Opening;
        Ok(())
    }

    pub(super) fn open(&mut self, link: &mut Transport, target: OpenTarget) -> Result<()> {
        let backend = self.options.backend;
        let cwd = self.options.cwd.clone();
        let launch = self.options.launch.clone();
        let tool_labels = self.options.tool_labels;
        self.submit(
            link,
            move |operation| Command::Open {
                operation,
                backend,
                cwd,
                launch,
                tool_labels,
                target,
            },
            Submitted::Open,
        )?;
        self.phase = Phase::Opening;
        Ok(())
    }

    pub(super) fn list(&mut self, link: &mut Transport) -> Result<()> {
        if let Some(submitted) = self.submitted.take() {
            let operation = match submitted {
                Submitted::Archive { operation, .. } | Submitted::Open(operation) => operation,
            };
            self.client.forget_operation(&operation);
        }
        let outgoing = self.client.read(
            link.connection,
            Command::SavedOpen {
                backend: self.options.backend,
                cwd: self.options.cwd.clone(),
                codex_socket: self.options.launch.codex_socket.clone(),
                purpose: if matches!(self.options.intent, Intent::Continue) {
                    SavedPurpose::Continue
                } else {
                    SavedPurpose::Picker
                },
            },
        )?;
        self.read = Some(outgoing.id());
        link.send(outgoing)?;
        self.error = None;
        self.phase = Phase::Listing;
        Ok(())
    }

    pub(super) fn page(
        &mut self,
        link: &mut Transport,
        direction: SavedDirection,
        landing: Landing,
    ) -> Result<()> {
        let Some(picker) = &mut self.picker else {
            return Ok(());
        };
        picker.landing = landing;
        let outgoing = self.client.read(
            link.connection,
            Command::SavedPage {
                scope: picker.page.scope.clone(),
                direction,
            },
        )?;
        self.read = Some(outgoing.id());
        link.send(outgoing)?;
        self.error = None;
        self.phase = Phase::Listing;
        Ok(())
    }

    pub(super) fn saved_page(&mut self, link: &mut Transport, page: SavedPage) -> Result<()> {
        if matches!(self.options.intent, Intent::Continue) {
            if let Some(row) = page.rows.first() {
                return self.open(
                    link,
                    OpenTarget::Choice {
                        scope: page.scope.clone(),
                        choice: row.choice.clone(),
                    },
                );
            }
            if self.options.backend != Backend::Codex {
                return self.create(link);
            }
        }
        if page.rows.is_empty() {
            if self.picker.take().is_some() {
                return self.list(link);
            }
            self.phase = Phase::Empty;
            return Ok(());
        }
        let landing = self
            .picker
            .as_ref()
            .filter(|picker| picker.page.scope == page.scope)
            .map_or(Landing::First, |picker| picker.landing);
        let selected = landing.index(page.rows.len());
        self.picker = Some(Picker {
            page,
            selected,
            top: 0,
            confirm: false,
            stale: false,
            landing: Landing::First,
        });
        self.phase = Phase::Picking;
        self.error = None;
        self.submitted = None;
        Ok(())
    }

    pub(super) fn submit(
        &mut self,
        link: &mut Transport,
        build: impl FnOnce(ClientOperation) -> Command,
        kind: impl FnOnce(ClientOperation) -> Submitted,
    ) -> Result<()> {
        let mut origin = None;
        let outgoing = self.client.submit(link.connection, |operation| {
            origin = Some(operation.clone());
            build(operation)
        })?;
        self.submitted = Some(kind(origin.expect("explicit operation")));
        self.error = None;
        link.send(outgoing)
    }
}
