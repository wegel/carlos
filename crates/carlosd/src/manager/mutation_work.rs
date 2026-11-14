//! Validate bounded typed mutations before reserving retry space or dispatching native work.

use carlos_core::engine::EngineCommand;
use carlos_proto::commands::Command;
use carlos_proto::submissions::SubmittedText;

use crate::retry::Class;
use crate::transport::wire::error;

pub(super) type Prepared = (
    carlos_proto::ids::SessionId,
    carlos_proto::submissions::ClientOperation,
    EngineCommand,
    Option<SubmittedText>,
    Class,
);

pub(super) fn prepare(command: Command) -> Result<Prepared, carlos_proto::commands::RpcError> {
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
        command @ (Command::Rename { .. } | Command::Rewind { .. }) => {
            return prepare_edit(command)
        }
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

fn prepare_edit(command: Command) -> Result<Prepared, carlos_proto::commands::RpcError> {
    let (session, operation, work, recovery) = match command {
        Command::Rename {
            session,
            operation,
            name,
        } => {
            if name.len() > crate::limits::NAME_BYTES {
                return Err(error(-32602, "name exceeds four KiB; nothing sent"));
            }
            (
                session,
                operation.clone(),
                EngineCommand::Rename { operation, name },
                None,
            )
        }
        Command::Rewind {
            session,
            operation,
            entry,
            text,
        } => {
            if text.len() > crate::limits::SEND_BYTES {
                return Err(error(-32602, "edited prompt exceeds 512 KiB; nothing sent"));
            }
            let recovery = Some(SubmittedText {
                operation: Some(operation.clone()),
                text: text.clone(),
            });
            (
                session,
                operation.clone(),
                EngineCommand::Rewind {
                    operation,
                    entry,
                    text,
                },
                recovery,
            )
        }
        _ => unreachable!("typed edit route"),
    };
    Ok((session, operation, work, recovery, Class::Ordinary))
}
