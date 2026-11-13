//! Fence one outstanding mutating backend wait while reads and native facts keep flowing.

use carlos_proto::ids::SessionId;

use super::command_work::{BackendCall, BackendWork, CommandError, Finish};
use super::state::SessionState;
use crate::backend::BackendKind;

#[derive(Debug, Default)]
pub(super) struct CommandState {
    issued: u64,
    pending: Option<u64>,
}

pub(super) struct WorkGuard {
    session: SessionId,
    thread: String,
    token: u64,
    pub(super) kind: BackendKind,
    epoch: u64,
    generation: u64,
    turn: Option<String>,
}

impl SessionState {
    pub(super) fn retire_backend_command(&mut self) {
        self.commands.pending = None;
    }

    pub(super) fn command_idle(&self) -> Result<(), CommandError> {
        if self.commands.pending.is_some() {
            Err(CommandError::Busy { submitted: None })
        } else {
            Ok(())
        }
    }

    pub(super) fn backend_work(
        &mut self,
        kind: BackendKind,
        call: BackendCall,
        finish: Finish,
        steer: bool,
    ) -> BackendWork {
        self.commands.issued = self
            .commands
            .issued
            .checked_add(1)
            .expect("command token exhausted");
        let token = self.commands.issued;
        self.commands.pending = Some(token);
        let (epoch, generation) = self.command_connection(kind);
        BackendWork {
            call,
            finish,
            guard: WorkGuard {
                session: self.events.id.clone(),
                thread: self.thread_id.clone(),
                token,
                kind,
                epoch,
                generation,
                turn: steer.then(|| self.active_turn_id.clone()).flatten(),
            },
        }
    }

    pub(super) fn accept_command_completion(&mut self, guard: &WorkGuard) -> bool {
        if guard.session != self.events.id || self.commands.pending != Some(guard.token) {
            return false;
        }
        self.commands.pending = None;
        let (epoch, generation) = self.command_connection(guard.kind);
        self.thread_id == guard.thread
            && epoch == guard.epoch
            && generation == guard.generation
            && guard
                .turn
                .as_ref()
                .is_none_or(|turn| self.active_turn_id.as_ref() == Some(turn))
            && match guard.kind {
                BackendKind::Codex => !self.codex_scope().closed(),
                BackendKind::Claude => self.claude.phase != super::claude::ClaudePhase::Closed,
                BackendKind::Grok => true,
            }
    }

    fn command_connection(&self, kind: BackendKind) -> (u64, u64) {
        match kind {
            BackendKind::Codex => (self.codex_scope().epoch, self.codex_connection.generation()),
            BackendKind::Claude => (self.claude.epoch, self.claude.generation),
            BackendKind::Grok => (0, 0),
        }
    }
}
