//! Resolve exact logical catalogue addresses before the explicit legacy import boundary.

use carlos_proto::commands::RpcError;
use carlos_proto::ids::{SavedChoiceId, SavedScopeId, SessionId};
use carlos_proto::session::Backend;
use carlos_proto::startup::OpenTarget;

use super::Target;
use crate::manager::Manager;
use crate::transport::wire::error;

impl Manager {
    pub(super) fn resolve_target(
        &self,
        peer: u64,
        backend: Backend,
        cwd: String,
        target: OpenTarget,
    ) -> Result<Target, RpcError> {
        match target {
            OpenTarget::Resume { address } => {
                if let Some(saved) = address
                    .parse::<SessionId>()
                    .ok()
                    .and_then(|id| self.sessions.get(&id))
                    .map(|session| &session.saved)
                {
                    return Ok(Target {
                        known: Some(saved.id.clone()),
                        backend: saved.backend,
                        cwd: saved.cwd.clone(),
                        native: saved.native_thread.clone(),
                        name: saved.name.clone(),
                        requested_cwd: None,
                        scope: None,
                    });
                }
                if address.is_empty() || address.len() > 4096 {
                    return Err(error(
                        -32602,
                        "explicit saved-session address is missing or exceeds four KiB",
                    ));
                }
                Ok(Target {
                    known: None,
                    backend,
                    cwd,
                    native: Some(address),
                    name: None,
                    requested_cwd: None,
                    scope: None,
                })
            }
            OpenTarget::Choice { scope, choice } => {
                self.resolve_choice(peer, backend, cwd, scope, choice)
            }
        }
    }

    fn resolve_choice(
        &self,
        peer: u64,
        backend: Backend,
        cwd: String,
        scope: SavedScopeId,
        choice: SavedChoiceId,
    ) -> Result<Target, RpcError> {
        let scope = self
            .startup
            .scopes
            .get(&peer)
            .filter(|current| current.id == scope && current.backend == backend)
            .ok_or_else(|| {
                error(
                    -32012,
                    "saved-session picker expired or belongs to another backend",
                )
            })?;
        let row = scope
            .rows
            .iter()
            .find(|(current, _)| current == &choice)
            .map(|(_, row)| row)
            .ok_or_else(|| error(-32012, "saved-session choice expired; refresh the picker"))?;
        Ok(Target {
            known: None,
            backend,
            cwd: scope.cwd.clone(),
            native: Some(row.id.clone()),
            name: row.name.clone(),
            requested_cwd: Some(cwd),
            scope: Some(scope.id.clone()),
        })
    }
}
