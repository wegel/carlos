//! Authenticate once, bind operation counters, and route only supported daemon methods.

use std::time::Instant;

use carlos_core::engine::EngineCommand;
use carlos_proto::commands::{check_protocol, Command, Request, RequestId, Sessions};
use carlos_proto::submissions::{OperationResult, OperationState, SubmittedText};

use super::Manager;
use crate::retry::{Admission, Class, Resume};
use crate::transport::wire::error;

impl Manager {
    pub(super) fn command(&mut self, peer: u64, request: Request) {
        let Request { id, command, .. } = request;
        let Some(connection) = self.peers.get(&peer) else {
            return;
        };
        if matches!(command, Command::Hello { .. }) {
            self.hello(peer, id, command);
            return;
        }
        let Some(client) = connection.client.clone() else {
            self.fail(peer, id, error(-32002, "hello is required before commands"));
            return;
        };
        if let Err(reason) = self.retry.touch(&client, Instant::now()) {
            self.fail(peer, id, reason);
            return;
        }
        match command {
            Command::Create { .. } => self.create(peer, id, command, client),
            Command::Send { .. }
            | Command::SetRalph { .. }
            | Command::Approve { .. }
            | Command::Interrupt { .. } => self.mutate(peer, id, command, client),
            Command::Watch {} => {
                self.peers.get_mut(&peer).expect("peer").list = true;
                let sessions = self
                    .sessions
                    .values()
                    .map(|session| session.info.clone())
                    .collect();
                self.reply(peer, id, Sessions { sessions });
            }
            Command::Operation { operation } => {
                match self.retry.read(&client, &operation, Instant::now()) {
                    Ok(result) => self.reply(peer, id, result),
                    Err(reason) => self.fail(peer, id, reason),
                }
            }
            command => self.read_command(peer, id, command),
        }
    }

    fn hello(&mut self, peer: u64, id: RequestId, command: Command) {
        let Command::Hello {
            protocol,
            client,
            token,
            resume,
            resume_token,
            instance,
        } = command
        else {
            unreachable!()
        };
        if self.peers[&peer].client.is_some() {
            self.fail(peer, id, error(-32002, "connection already greeted"));
            return;
        }
        if let Err(reason) = check_protocol(protocol) {
            self.fail(peer, id, reason);
            return;
        }
        if client.len() > 256 || !token.is_empty() {
            self.fail(
                peer,
                id,
                error(
                    -32602,
                    "use a bounded client name and Unix peer credentials",
                ),
            );
            return;
        }
        let resume = match (resume, resume_token, instance) {
            (None, None, None) => None,
            (Some(client), Some(secret), Some(instance)) => Some(Resume {
                client,
                secret,
                instance,
            }),
            _ => {
                self.fail(
                    peer,
                    id,
                    error(
                        -32602,
                        "resume requires client, private token and daemon instance",
                    ),
                );
                return;
            }
        };
        match self.retry.hello(resume, Instant::now()) {
            Ok(hello) => {
                let connection = self.peers.get_mut(&peer).expect("peer");
                connection.client = Some(hello.client.clone());
                connection.replay_allowed = hello.resumed;
                self.reply(peer, id, hello);
            }
            Err(reason) => self.fail(peer, id, reason),
        }
    }

    fn mutate(
        &mut self,
        peer: u64,
        id: RequestId,
        command: Command,
        client: carlos_proto::ids::ClientId,
    ) {
        let canonical = serde_json::to_vec(&command).expect("typed command");
        let (session, operation, work, recovery, class) = match prepare(command) {
            Ok(parts) => parts,
            Err(reason) => {
                self.fail(peer, id, reason);
                return;
            }
        };
        let result = match self.retry.admit(
            &client,
            operation,
            session.clone(),
            canonical,
            class,
            Instant::now(),
        ) {
            Ok(Admission::Existing(result)) => {
                self.reply(peer, id, result);
                return;
            }
            Ok(Admission::New(result)) => result,
            Err(reason) => {
                self.fail(peer, id, reason);
                return;
            }
        };
        let admitted = self
            .sessions
            .get(&session)
            .and_then(|session| session.live.as_ref())
            .ok_or_else(|| error(-32012, "session has no live owner; subscribe to restore it"))
            .and_then(|live| live.handle.request(work));
        match admitted {
            Ok(_) => self.reply(peer, id, result),
            Err(reason) => {
                let result = OperationResult {
                    state: OperationState::Rejected {
                        message: reason.message,
                        recovery,
                    },
                    ..result
                };
                self.result(result.clone());
                self.reply(peer, id, result);
            }
        }
    }
}

type Prepared = (
    carlos_proto::ids::SessionId,
    carlos_proto::submissions::ClientOperation,
    EngineCommand,
    Option<SubmittedText>,
    Class,
);

fn prepare(command: Command) -> Result<Prepared, carlos_proto::commands::RpcError> {
    let parts = match command {
        Command::Send {
            session,
            operation,
            text,
            policy,
        } => return prepare_send(session, operation, text, policy),
        Command::SetRalph {
            session,
            operation,
            on,
        } => (
            session,
            operation.clone(),
            EngineCommand::SetRalph { operation, on },
            None,
            Class::Ordinary,
        ),
        Command::Approve {
            session,
            operation,
            approval,
            answer,
        } => (
            session,
            operation.clone(),
            EngineCommand::Approve {
                operation,
                card: approval,
                answer,
            },
            None,
            Class::Control,
        ),
        Command::Interrupt {
            session,
            operation,
            turn,
        } => (
            session,
            operation.clone(),
            EngineCommand::Interrupt { operation, turn },
            None,
            Class::Control,
        ),
        _ => unreachable!("mutation route"),
    };
    Ok(parts)
}

fn prepare_send(
    session: carlos_proto::ids::SessionId,
    operation: carlos_proto::submissions::ClientOperation,
    text: String,
    policy: carlos_proto::commands::SendPolicy,
) -> Result<Prepared, carlos_proto::commands::RpcError> {
    if text.len() > crate::limits::SEND_BYTES {
        return Err(error(-32602, "send exceeds 512 KiB; nothing dispatched"));
    }
    let recovery = Some(SubmittedText {
        operation: Some(operation.clone()),
        text: text.clone(),
    });
    Ok((
        session,
        operation.clone(),
        EngineCommand::Send {
            operation,
            text,
            policy,
        },
        recovery,
        Class::Ordinary,
    ))
}
