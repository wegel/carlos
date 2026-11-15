//! Archive only one captured Codex address; confirmed replies and uncertain writes stay distinct.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use serde_json::{json, Value};

use super::remaining;
use crate::backend::BackendClient;
use crate::backend_commands::{BackendCommands, CommandHandle};
use crate::codex_connection::CodexClient;
use crate::dispatch_guard::{DispatchGuard, DispatchRetired};
use crate::launch_cancel::LaunchCancellation;
use crate::protocol_params::{extract_result_object, params_initialize};

/// The manager captures this native address from its own private picker, never from archive JSON.
pub struct ArchiveRequest {
    pub native_thread: String,
    pub codex_socket: Option<PathBuf>,
}

/// A successful native reply confirms archive; missing replies cannot authorize a second write.
pub enum ArchiveResult {
    Accepted,
    Rejected(String),
    Uncertain(String),
}

/// Run the temporary adapter and its shutdown entirely on the admitted lifecycle worker.
pub fn archive_codex(
    request: ArchiveRequest,
    cancel: &LaunchCancellation,
    deadline: Instant,
) -> ArchiveResult {
    if request.native_thread.is_empty() || request.native_thread.len() > 4096 {
        return ArchiveResult::Rejected("saved native address is missing or oversized".into());
    }
    match prepare(&request, cancel, deadline) {
        Ok((client, commands)) => {
            let timeout = match remaining(cancel, deadline) {
                Ok(timeout) => timeout,
                Err(error) => return ArchiveResult::Rejected(error.to_string()),
            };
            let result = BackendCommands::call(
                &*commands,
                "thread/archive",
                json!({"threadId":request.native_thread}),
                timeout,
            );
            drop(client);
            match result {
                Ok(response) => parse(&response),
                Err(error) if error.downcast_ref::<DispatchRetired>().is_some() => {
                    ArchiveResult::Rejected(error.to_string())
                }
                Err(error) => ArchiveResult::Uncertain(format!(
                    "native archive could not be confirmed: {error}"
                )),
            }
        }
        Err(error) => ArchiveResult::Rejected(error.to_string()),
    }
}

fn prepare(
    request: &ArchiveRequest,
    cancel: &LaunchCancellation,
    deadline: Instant,
) -> Result<(CodexClient, CommandHandle)> {
    remaining(cancel, deadline)?;
    let client = CodexClient::start(request.codex_socket.clone())?;
    let commands = client.capture_commands()?;
    let abort = commands.clone();
    cancel.register(move || abort.abort())?;
    let response = BackendCommands::call(
        &*commands,
        "initialize",
        params_initialize(),
        remaining(cancel, deadline)?,
    )?;
    extract_result_object(&response)?;
    client.notify("initialized", json!({}))?;
    let captured = cancel.clone();
    let commands = commands.guarded(DispatchGuard::new(move || {
        captured.check().is_ok() && Instant::now() < deadline
    }))?;
    Ok((client, commands))
}

fn parse(response: &str) -> ArchiveResult {
    let Ok(response) = serde_json::from_str::<Value>(response) else {
        return ArchiveResult::Uncertain("native archive returned an invalid reply".into());
    };
    if let Some(error) = response.get("error") {
        return ArchiveResult::Rejected(
            error["message"]
                .as_str()
                .unwrap_or("native archive was rejected")
                .to_owned(),
        );
    }
    if response.get("result").is_some() {
        ArchiveResult::Accepted
    } else {
        ArchiveResult::Uncertain("native archive reply omitted acceptance".into())
    }
}
