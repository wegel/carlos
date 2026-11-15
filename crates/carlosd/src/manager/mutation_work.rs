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
        Command::ToolLabelsSet {
            session,
            operation,
            expected,
            enabled,
            save_default,
        } => (
            session,
            operation.clone(),
            EngineCommand::ToolLabelsSet {
                operation,
                expected,
                enabled,
                save_default,
            },
            None,
            Class::Ordinary,
        ),
        settings @ Command::SettingsSet { .. } => return prepare_settings(settings),
        command @ Command::TaskStop { .. } => return prepare_task_stop(command),
        command @ Command::QueueChange { .. } => return prepare_queue(command),
        command @ (Command::GoalSet { .. } | Command::GoalClear { .. }) => {
            return prepare_goal(command)
        }
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
        control @ (Command::Approve { .. } | Command::Interrupt { .. }) => {
            return Ok(prepare_control(control))
        }
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

fn prepare_goal(command: Command) -> Result<Prepared, carlos_proto::commands::RpcError> {
    Ok(match command {
        Command::GoalSet {
            session,
            operation,
            expected,
            change,
        } => {
            if change
                .objective
                .as_ref()
                .is_some_and(|text| text.len() > carlos_proto::goals::MAX_OBJECTIVE_BYTES)
            {
                return Err(error(-32602, "goal objective exceeds 32 KiB; nothing sent"));
            }
            (
                session,
                operation.clone(),
                EngineCommand::GoalSet {
                    operation,
                    expected,
                    change,
                },
                None,
                Class::Ordinary,
            )
        }
        Command::GoalClear {
            session,
            operation,
            expected,
        } => (
            session,
            operation.clone(),
            EngineCommand::GoalClear {
                operation,
                expected,
            },
            None,
            Class::Ordinary,
        ),
        _ => unreachable!("typed goal mutation"),
    })
}

fn prepare_queue(command: Command) -> Result<Prepared, carlos_proto::commands::RpcError> {
    match command {
        Command::QueueChange {
            session,
            operation,
            expected,
            change,
        } => {
            if change
                .text()
                .is_some_and(|text| text.len() > carlos_proto::queue::MAX_TEXT_BYTES)
            {
                return Err(error(-32602, "queued text exceeds 512 KiB; nothing sent"));
            }
            let recovery = change.text().map(|text| SubmittedText {
                operation: Some(operation.clone()),
                text: text.into(),
            });
            Ok((
                session,
                operation.clone(),
                EngineCommand::QueueChange {
                    operation,
                    expected,
                    change,
                },
                recovery,
                Class::Ordinary,
            ))
        }
        _ => unreachable!("typed queue mutation"),
    }
}

fn prepare_task_stop(command: Command) -> Result<Prepared, carlos_proto::commands::RpcError> {
    Ok(match command {
        Command::TaskStop {
            window: _,
            session,
            operation,
            task,
            expected_run,
        } => (
            session,
            operation.clone(),
            EngineCommand::TaskStop {
                window: None,
                operation,
                task,
                expected_run,
            },
            None,
            Class::Control,
        ),
        _ => unreachable!("typed task stop route"),
    })
}

fn prepare_control(command: Command) -> Prepared {
    match command {
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
        _ => unreachable!("typed control command"),
    }
}

fn prepare_settings(command: Command) -> Result<Prepared, carlos_proto::commands::RpcError> {
    let Command::SettingsSet {
        session,
        operation,
        expected,
        change,
        save_default,
    } = command
    else {
        unreachable!("typed settings change")
    };
    Ok((
        session,
        operation.clone(),
        EngineCommand::SettingsSet {
            operation,
            expected,
            change,
            save_default,
        },
        None,
        Class::Ordinary,
    ))
}
