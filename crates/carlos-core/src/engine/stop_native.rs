//! Interrupt only the selected attached Codex conversation before releasing its Carlos owner.

use std::time::Duration;

use carlos_proto::commands::RpcError;
use serde_json::json;

use crate::backend_commands::CommandHandle;

use super::owner::Owner;
use crate::backend::BackendKind;
use crate::session::control_work::ControlFailure;

impl Owner {
    pub(super) fn stop_attached_turn(&mut self) -> Result<(), RpcError> {
        if self.client.kind() != BackendKind::Codex
            || self.session.codex_connection().socket().is_none()
        {
            return Ok(());
        }
        self.drain_native();
        self.drain_completions();
        if self.native.closed || self.session.codex_scope().closed() {
            return Err(uncertain(
                "attached connection is closed; native work could not be checked",
            ));
        }
        let commands = self
            .client
            .capture_commands()
            .map_err(|error| uncertain(&error.to_string()))?;
        if commands.connection_generation() != Some(self.session.codex_connection().generation()) {
            return Err(uncertain("captured native connection changed before stop"));
        }
        self.session.refresh_wire_turn();
        let Some(turn) = self
            .session
            .events
            .current_turn
            .as_ref()
            .map(|turn| turn.id.clone())
        else {
            return if self.workers.ordinary_busy {
                Err(uncertain(
                    "a native send is still in flight without a confirmed turn",
                ))
            } else {
                confirm_idle(&commands, self.session.thread_id())
            };
        };
        let work = self
            .session
            .prepare_engine_interrupt(self.client.as_ref(), &turn)
            .map_err(|error| uncertain(&error.to_string()))?;
        let completion = work.execute_timeout(&commands, Duration::from_secs(2));
        self.session
            .finish_engine_control(self.client.as_ref(), completion)
            .map_err(|error| {
                let message = match error {
                    ControlFailure::Invalid(message) | ControlFailure::Native(message) => message,
                    ControlFailure::Retired => {
                        "captured native turn or connection was retired".into()
                    }
                };
                uncertain(&message)
            })
    }
}

fn uncertain(message: &str) -> RpcError {
    RpcError {
        code: -32012,
        message: format!(
            "Attached Codex stop was not confirmed: {}",
            crate::protocol_events::bounded_diagnostic(message)
        ),
    }
}

fn confirm_idle(commands: &CommandHandle, thread: &str) -> Result<(), RpcError> {
    let response = commands
        .call(
            "thread/read",
            json!({"threadId":thread,"includeTurns":false}),
            Duration::from_secs(2),
        )
        .and_then(|line| crate::protocol_params::extract_result_object(&line))
        .map_err(|error| uncertain(&error.to_string()))?;
    let current = &response["result"]["thread"];
    if current["id"] == thread
        && matches!(
            current["status"]["type"].as_str(),
            Some("idle" | "notLoaded")
        )
    {
        Ok(())
    } else {
        Err(uncertain(
            "the native session is not confirmed idle and no safe turn identifier is known",
        ))
    }
}
