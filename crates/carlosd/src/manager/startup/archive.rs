//! Admit explicit archive consent against one opaque picker choice and retain its private outcome.

use std::time::Instant;

use carlos_core::saved_sessions::{archive_codex, ArchiveRequest, ArchiveResult};
use carlos_proto::commands::{Command, RequestId, RpcError};
use carlos_proto::ids::{ClientId, SavedChoiceId, SavedScopeId};
use carlos_proto::session::Backend;
use carlos_proto::submissions::{OperationResult, OperationState};

use super::{Completed, JobKind};
use crate::manager::Manager;
use crate::retry::{Admission, Class};
use crate::transport::wire::error;

pub(super) struct ArchiveJob {
    pub result: OperationResult,
    pub scope: SavedScopeId,
    pub choice: SavedChoiceId,
}

impl Manager {
    pub(super) fn archive_command(
        &mut self,
        peer: u64,
        id: RequestId,
        command: Command,
        client: ClientId,
    ) {
        let canonical = serde_json::to_vec(&command).expect("typed archive");
        let Command::SavedArchive {
            operation,
            scope,
            choice,
        } = command
        else {
            unreachable!("typed archive")
        };
        match self
            .retry
            .existing(&client, &operation, &canonical, Instant::now())
        {
            Ok(Some(result)) => return self.reply(peer, id, result),
            Ok(None) => {}
            Err(reason) => return self.fail(peer, id, reason),
        }
        let request = match self.archive_request(peer, &scope, &choice) {
            Ok(request) => request,
            Err(reason) => return self.fail(peer, id, reason),
        };
        if let Err(reason) = self.startup_admission(peer) {
            return self.fail(peer, id, reason);
        }
        let result = match self.retry.admit(
            &client,
            operation,
            None,
            canonical,
            Class::Ordinary,
            Instant::now(),
        ) {
            Ok(Admission::Existing(result)) => return self.reply(peer, id, result),
            Ok(Admission::New(result)) => result,
            Err(reason) => return self.fail(peer, id, reason),
        };
        self.launch_archive(peer, id, result, scope, choice, request);
    }

    fn archive_request(
        &self,
        peer: u64,
        scope: &SavedScopeId,
        choice: &SavedChoiceId,
    ) -> Result<ArchiveRequest, RpcError> {
        let page = self
            .startup
            .scopes
            .get(&peer)
            .filter(|page| &page.id == scope)
            .ok_or_else(|| error(-32012, "saved-session picker expired; open it again"))?;
        let (_, row) = page
            .rows
            .iter()
            .find(|(key, _)| key == choice)
            .ok_or_else(|| error(-32012, "saved-session choice expired; refresh the picker"))?;
        if page.backend != Backend::Codex {
            return Err(error(
                -32602,
                "this backend does not support saved-session archive",
            ));
        }
        Ok(ArchiveRequest {
            native_thread: row.id.clone(),
            codex_socket: self.codex_socket.clone(),
        })
    }

    fn launch_archive(
        &mut self,
        peer: u64,
        id: RequestId,
        result: OperationResult,
        scope: SavedScopeId,
        choice: SavedChoiceId,
        request: ArchiveRequest,
    ) {
        let job = ArchiveJob {
            result: result.clone(),
            scope,
            choice,
        };
        self.start_startup_job(
            peer,
            id.clone(),
            JobKind::Archive(Box::new(job)),
            move |cancel, deadline| {
                Ok(Completed::Archived(archive_codex(
                    request, cancel, deadline,
                )))
            },
        );
        self.reply(peer, id, result);
    }

    pub(super) fn finish_archive(&mut self, job: ArchiveJob, outcome: ArchiveResult) {
        let mut result = job.result;
        result.state = match outcome {
            ArchiveResult::Accepted => {
                for page in self
                    .startup
                    .scopes
                    .values_mut()
                    .filter(|page| page.id == job.scope)
                {
                    page.rows.retain(|(choice, _)| choice != &job.choice);
                }
                OperationState::Accepted {
                    entry: None,
                    notice: Some("Saved session archived".into()),
                }
            }
            ArchiveResult::Rejected(message) => OperationState::Rejected {
                message,
                recovery: None,
            },
            ArchiveResult::Uncertain(message) => OperationState::Uncertain {
                message,
                recovery: None,
            },
        };
        self.result(result);
    }
}
