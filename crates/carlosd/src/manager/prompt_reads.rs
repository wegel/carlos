//! Forward private prompt cursors through the existing bounded owner read lane.

use carlos_core::engine::EngineCommand;
use carlos_core::session::prompt_reads::PromptCommand;
use carlos_proto::commands::{Command, RequestId};

use super::Manager;

impl Manager {
    pub(super) fn prompt_read_command(&mut self, peer: u64, id: RequestId, command: Command) {
        let (session, command) = match command {
            Command::PromptSelect {
                session,
                source,
                anchor,
                direction,
            } => (
                session,
                PromptCommand::Select {
                    source,
                    anchor,
                    direction,
                },
            ),
            Command::PromptText {
                session,
                prompt,
                offset,
                limit,
            } => (
                session,
                PromptCommand::Text {
                    prompt,
                    offset,
                    limit: limit.unwrap_or(65536).clamp(4, 65536) as usize,
                },
            ),
            _ => unreachable!("typed prompt read"),
        };
        self.rpc_read(peer, id, session, EngineCommand::Prompts(command));
    }
}
