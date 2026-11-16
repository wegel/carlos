//! Validate bounded typed mutations before reserving retry space or dispatching native work.

use carlos_core::engine::EngineCommand;
use carlos_proto::commands::Command;
use carlos_proto::submissions::SubmittedText;

use crate::command_info::describe;
use crate::retry::Class;
use crate::transport::wire::error;

/// Carry validated native work and its shared command-description retry class.
pub(super) struct Prepared {
    pub session: carlos_proto::ids::SessionId,
    pub operation: carlos_proto::submissions::ClientOperation,
    pub work: EngineCommand,
    pub recovery: Option<SubmittedText>,
    pub class: Class,
}

type Work = (
    carlos_proto::ids::SessionId,
    carlos_proto::submissions::ClientOperation,
    EngineCommand,
    Option<SubmittedText>,
);

pub(super) fn prepare(command: Command) -> Result<Prepared, carlos_proto::commands::RpcError> {
    let class = describe(&command).retry.expect("mutation retry class");
    let (session, operation, work, recovery) = match command {
        labels @ Command::ToolLabelsSet { .. } => prepare_tool_labels(labels),
        context @ Command::ContextSet { .. } => prepare_context(context),
        settings @ Command::SettingsSet { .. } => prepare_settings(settings),
        command @ Command::TaskStop { .. } => prepare_task_stop(command),
        command @ Command::QueueChange { .. } => prepare_queue(command),
        command @ (Command::GoalSet { .. } | Command::GoalClear { .. }) => prepare_goal(command),
        Command::Send {
            session,
            operation,
            text,
            policy,
        } => prepare_send(session, operation, text, policy),
        ralph @ (Command::SetRalph { .. } | Command::ToggleRalph { .. }) => prepare_ralph(ralph),
        control @ (Command::Approve { .. }
        | Command::Interrupt { .. }
        | Command::Reconnect { .. }) => Ok(prepare_control(control)),
        command @ (Command::Rename { .. } | Command::Rewind { .. }) => prepare_edit(command),
        _ => unreachable!("mutation route"),
    }?;
    Ok(Prepared {
        session,
        operation,
        work,
        recovery,
        class,
    })
}

fn prepare_send(
    session: carlos_proto::ids::SessionId,
    operation: carlos_proto::submissions::ClientOperation,
    text: String,
    policy: carlos_proto::commands::SendPolicy,
) -> Result<Work, carlos_proto::commands::RpcError> {
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
    ))
}

fn prepare_edit(command: Command) -> Result<Work, carlos_proto::commands::RpcError> {
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
    Ok((session, operation, work, recovery))
}

fn prepare_goal(command: Command) -> Result<Work, carlos_proto::commands::RpcError> {
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
        ),
        _ => unreachable!("typed goal mutation"),
    })
}

fn prepare_queue(command: Command) -> Result<Work, carlos_proto::commands::RpcError> {
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
            ))
        }
        _ => unreachable!("typed queue mutation"),
    }
}

fn prepare_task_stop(command: Command) -> Result<Work, carlos_proto::commands::RpcError> {
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
        ),
        _ => unreachable!("typed task stop route"),
    })
}

fn prepare_control(command: Command) -> Work {
    match command {
        Command::Reconnect { session, operation } => (
            session,
            operation.clone(),
            EngineCommand::Reconnect { operation },
            None,
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
        ),
        _ => unreachable!("typed control command"),
    }
}

fn prepare_context(command: Command) -> Result<Work, carlos_proto::commands::RpcError> {
    let Command::ContextSet {
        session,
        operation,
        choice,
    } = command
    else {
        unreachable!("typed context change");
    };
    Ok((
        session,
        operation.clone(),
        EngineCommand::ContextSet { operation, choice },
        None,
    ))
}

fn prepare_settings(command: Command) -> Result<Work, carlos_proto::commands::RpcError> {
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
    ))
}

fn prepare_tool_labels(command: Command) -> Result<Work, carlos_proto::commands::RpcError> {
    let Command::ToolLabelsSet {
        session,
        operation,
        expected,
        enabled,
        save_default,
    } = command
    else {
        unreachable!("typed tool-label change");
    };
    Ok((
        session,
        operation.clone(),
        EngineCommand::ToolLabelsSet {
            operation,
            expected,
            enabled,
            save_default,
        },
        None,
    ))
}

fn prepare_ralph(command: Command) -> Result<Work, carlos_proto::commands::RpcError> {
    let (session, operation, work) = match command {
        Command::SetRalph {
            session,
            operation,
            on,
            options,
        } => {
            if !on && options.is_some() {
                return Err(error(-32602, "prompt options require Ralph on"));
            }
            (
                session,
                operation.clone(),
                EngineCommand::SetRalph {
                    operation,
                    on,
                    options,
                },
            )
        }
        Command::ToggleRalph {
            session,
            operation,
            options,
        } => (
            session,
            operation.clone(),
            EngineCommand::ToggleRalph { operation, options },
        ),
        _ => unreachable!("typed Ralph request"),
    };
    Ok((session, operation, work, None))
}

#[cfg(test)]
#[path = "mutation_work_tests.rs"]
mod tests;
