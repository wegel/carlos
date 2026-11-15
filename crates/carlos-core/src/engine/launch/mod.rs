//! Launch native adapters off the manager and transfer each session directly to its owner.

mod backends;
mod options;

use std::fmt;
use std::path::PathBuf;
use std::sync::mpsc;

use carlos_proto::ids::SessionId;
use carlos_proto::session::{Backend, SessionInfo};

use super::EngineHandle;
use crate::backend::{BackendClient, EventReceiver};
use crate::launch_cancel::LaunchCancellation;
use crate::session::state::SessionState;

pub use self::options::{validate_endpoint, LaunchOptions};

/// Restore only the selected native conversation; saved automatic work always needs a fresh choice.
pub struct RestoreSession {
    pub native_thread: String,
    pub ralph_was_on: bool,
}

/// Keep launch choices separate from commands addressed to a running session.
pub struct LaunchRequest {
    pub session: SessionId,
    pub backend: Backend,
    pub cwd: PathBuf,
    pub name: Option<String>,
    pub restore: Option<RestoreSession>,
    pub options: LaunchOptions,
}

/// Own the adapter until the manager transfers it to one session thread.
pub struct Launched {
    session: SessionState,
    backend: Box<dyn BackendClient>,
    events: EventReceiver,
}

/// Invalid choices start no helper; native failures and cancelled launches stay explicit.
#[derive(Debug)]
pub enum LaunchError {
    Invalid(String),
    Native(String),
    Cancelled,
}

/// Run filesystem and native startup outside both the session owner and the socket manager.
pub fn launch(
    mut request: LaunchRequest,
    cancel: &LaunchCancellation,
) -> Result<Launched, LaunchError> {
    cancel.check().map_err(|_| LaunchError::Cancelled)?;
    validate(&mut request)?;
    cancel.check().map_err(|_| LaunchError::Cancelled)?;
    let preferences = crate::runtime_preferences::load();
    let summary_override = request
        .options
        .native
        .reasoning_summary
        .as_ref()
        .map(|choice| choice.as_str().to_owned())
        .or_else(summary_override);
    cancel.check().map_err(|_| LaunchError::Cancelled)?;
    let mut launched =
        backends::start(&request, cancel, preferences, summary_override).map_err(|error| {
            if cancel.check().is_err() {
                LaunchError::Cancelled
            } else {
                LaunchError::Native(error.to_string())
            }
        })?;
    launched.session.events.id = request.session;
    launched
        .session
        .set_client_identity(request.backend, "local".into());
    launched.session.set_summary_cwd(request.cwd.clone());
    launched.session.apply_summary_setting(
        request
            .options
            .tool_labels
            .unwrap_or_else(crate::ui_preferences::load_summary_enabled),
    );
    launched
        .session
        .configure_ralph_options(request.cwd, None, None, None, None);
    if let Some(name) = request.name {
        launched.session.set_thread_name(Some(name.clone()));
        launched.session.events.launch_name = Some(name);
    }
    launched.session.events.ralph_restore_notice = request
        .restore
        .as_ref()
        .is_some_and(|saved| saved.ralph_was_on);
    cancel.check().map_err(|_| LaunchError::Cancelled)?;
    Ok(launched)
}

impl Launched {
    /// Read initial launch facts without consuming or numbering pending native changes.
    pub fn info(&mut self) -> SessionInfo {
        self.session.refresh_wire_turn();
        self.session.export_info()
    }

    /// Capture the live native address separately from its last confirmed catalogue entry.
    pub fn native_thread(&self) -> &str {
        self.session.thread_id()
    }

    /// Transfer the whole adapter without exposing mutable session fields to the daemon manager.
    pub fn spawn(self) -> (EngineHandle, mpsc::Receiver<()>) {
        EngineHandle::spawn_with_tool_labels(self.session, self.backend, self.events)
    }
}

fn summary_override() -> Option<String> {
    std::env::var("CARLOS_REASONING_SUMMARY")
        .ok()
        .map(|value| parse_summary_override(&value))
}

fn parse_summary_override(value: &str) -> String {
    match value.trim() {
        "auto" | "concise" | "detailed" | "none" => value.trim().to_owned(),
        _ => "auto".into(),
    }
}

fn validate(request: &mut LaunchRequest) -> Result<(), LaunchError> {
    options::validate_options(request.backend, &request.options)?;
    if !request.cwd.is_absolute() {
        return Err(LaunchError::Invalid(
            "session cwd must be an absolute directory".into(),
        ));
    }
    request.cwd = request
        .cwd
        .canonicalize()
        .map_err(|error| LaunchError::Invalid(format!("session cwd: {error}")))?;
    if !request.cwd.is_dir() || request.cwd.to_str().is_none() {
        return Err(LaunchError::Invalid(
            "session cwd must be a UTF-8 directory".into(),
        ));
    }
    if request.name.as_ref().is_some_and(|name| name.len() > 4096) {
        return Err(LaunchError::Invalid("session name exceeds four KiB".into()));
    }
    if request
        .restore
        .as_ref()
        .is_some_and(|saved| saved.native_thread.is_empty() || saved.native_thread.len() > 4096)
    {
        return Err(LaunchError::Invalid(
            "saved native thread id is missing or oversized; no new session was created".into(),
        ));
    }
    Ok(())
}

impl fmt::Display for LaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) | Self::Native(message) => formatter.write_str(message),
            Self::Cancelled => formatter.write_str("backend launch cancelled"),
        }
    }
}
impl std::error::Error for LaunchError {}

#[cfg(test)]
mod tests;
