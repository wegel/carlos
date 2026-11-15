//! Bind every displayed row to one peer's current captured native address.

use carlos_core::saved_sessions::{PageAnchor, PageDirection, SavedPage as NativePage};
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::{SavedChoiceId, SavedScopeId};
use carlos_proto::session::Backend;
use carlos_proto::startup::{SavedChoice, SavedDirection, SavedPage};

use super::workers::PageWork;
use super::{Job, JobKind, Scope};
use crate::manager::Manager;
use crate::transport::wire::error;

impl Manager {
    pub(super) fn saved_command(&mut self, peer: u64, id: RequestId, command: Command) {
        match command {
            Command::SavedOpen {
                backend,
                cwd,
                codex_socket,
                purpose,
            } => {
                let scope = opaque_id::<SavedScopeId>();
                let kind = JobKind::Page {
                    scope,
                    backend,
                    purpose,
                };
                let work = PageWork {
                    backend,
                    cwd,
                    requested_socket: codex_socket,
                    purpose,
                    direction: PageDirection::Newest,
                    configured_socket: self.codex_socket.clone(),
                };
                self.start_startup_job(peer, id, kind, move |cancel, deadline| {
                    work.run(cancel, deadline)
                });
            }
            Command::SavedPage { scope, direction } => self.saved_page(peer, id, scope, direction),
            Command::SavedClose { scope } => self.saved_close(peer, id, scope),
            _ => unreachable!("saved-session read"),
        }
    }

    fn saved_page(
        &mut self,
        peer: u64,
        id: RequestId,
        scope: SavedScopeId,
        direction: SavedDirection,
    ) {
        let Some(current) = self
            .startup
            .scopes
            .get(&peer)
            .filter(|current| current.id == scope)
        else {
            self.fail(
                peer,
                id,
                error(-32012, "saved-session picker expired; open it again"),
            );
            return;
        };
        if current.purpose != carlos_proto::startup::SavedPurpose::Picker {
            self.fail(
                peer,
                id,
                error(-32602, "continue lookup has no picker pages"),
            );
            return;
        }
        let direction = page_direction(current, direction);
        let Some(direction) = direction else {
            self.fail(peer, id, error(-32602, "no neighboring saved-session page"));
            return;
        };
        let kind = JobKind::Page {
            scope,
            backend: current.backend,
            purpose: current.purpose,
        };
        let work = PageWork {
            backend: current.backend,
            cwd: current.cwd.clone(),
            purpose: current.purpose,
            direction,
            configured_socket: self.codex_socket.clone(),
            requested_socket: None,
        };
        self.start_startup_job(peer, id, kind, move |cancel, deadline| {
            work.run(cancel, deadline)
        });
    }

    fn saved_close(&mut self, peer: u64, id: RequestId, scope: Option<SavedScopeId>) {
        if self
            .startup
            .scopes
            .get(&peer)
            .is_some_and(|current| scope.as_ref().is_none_or(|scope| scope == &current.id))
        {
            self.startup.scopes.remove(&peer);
        }
        let mut replies = Vec::new();
        for job in self
            .startup
            .jobs
            .values_mut()
            .filter(|job| job.peer == peer && !job.retired)
        {
            let captured = match &job.kind {
                JobKind::Page { scope, .. } => Some(scope),
                JobKind::Open(request) => request.target.scope.as_ref(),
            };
            if captured.is_some_and(|current| scope.as_ref().is_none_or(|scope| scope == current)) {
                job.retired = true;
                job.cancel.cancel();
                replies.push(job.id.clone());
            }
        }
        for pending in replies {
            self.fail(
                peer,
                pending,
                error(-32012, "saved-session read was closed"),
            );
        }
        self.reply(peer, id, ());
    }

    pub(super) fn finish_saved(&mut self, job: Job, cwd: String, page: NativePage) {
        let JobKind::Page {
            scope,
            backend,
            purpose,
        } = job.kind
        else {
            unreachable!("page worker")
        };
        let paged = purpose == carlos_proto::startup::SavedPurpose::Picker;
        let rows: Vec<_> = page
            .rows
            .into_iter()
            .map(|row| (opaque_id::<SavedChoiceId>(), row))
            .collect();
        let response = SavedPage {
            scope: scope.clone(),
            backend,
            rows: rows
                .iter()
                .map(|(choice, row)| SavedChoice {
                    choice: choice.clone(),
                    name: row.name.clone(),
                    preview: row.preview.clone(),
                    cwd: row.cwd.clone(),
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                })
                .collect(),
            has_newer: paged && page.has_newer,
            has_older: paged && page.has_older,
            can_archive: backend == Backend::Codex,
        };
        self.startup.scopes.insert(
            job.peer,
            Scope {
                id: scope,
                backend,
                cwd,
                purpose,
                rows,
                has_newer: paged && page.has_newer,
                has_older: paged && page.has_older,
            },
        );
        self.reply(job.peer, job.id, response);
    }
}

fn page_direction(current: &Scope, direction: SavedDirection) -> Option<PageDirection> {
    match direction {
        SavedDirection::Newest => Some(PageDirection::Newest),
        SavedDirection::Oldest => Some(PageDirection::Oldest),
        SavedDirection::Older if current.has_older => current
            .rows
            .last()
            .map(|(_, row)| PageDirection::Older(PageAnchor::from_row(row))),
        SavedDirection::Newer if current.has_newer => current
            .rows
            .first()
            .map(|(_, row)| PageDirection::Newer(PageAnchor::from_row(row))),
        _ => None,
    }
}

pub(super) fn opaque_id<T: std::str::FromStr>() -> T
where
    T::Err: std::fmt::Debug,
{
    uuid::Uuid::new_v4()
        .simple()
        .to_string()
        .parse()
        .expect("opaque UUID")
}
