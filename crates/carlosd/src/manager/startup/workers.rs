//! Keep filesystem checks and temporary native readers off the manager and session owners.

use std::path::PathBuf;
use std::time::Instant;

use carlos_core::engine::launch::validate_endpoint;
use carlos_core::launch_cancel::LaunchCancellation;
use carlos_core::saved_sessions::{read_page_until, PageDirection, SavedPageRequest};
use carlos_proto::session::Backend;
use carlos_proto::startup::SavedPurpose;

use super::Completed;

pub(super) struct PageWork {
    pub backend: Backend,
    pub cwd: String,
    pub requested_socket: Option<String>,
    pub configured_socket: Option<PathBuf>,
    pub purpose: SavedPurpose,
    pub direction: PageDirection,
}

impl PageWork {
    pub(super) fn run(
        self,
        cancel: &LaunchCancellation,
        deadline: Instant,
    ) -> Result<Completed, String> {
        let cwd = check(
            self.backend,
            &self.cwd,
            self.requested_socket.as_deref(),
            self.configured_socket.as_deref(),
            cancel,
            deadline,
        )?;
        let page = read_page_until(
            SavedPageRequest {
                backend: self.backend,
                cwd: PathBuf::from(&cwd),
                codex_socket: self.configured_socket,
                direction: self.direction,
                allow_grok_fallback: self.purpose == SavedPurpose::Picker,
            },
            cancel,
            deadline,
        )
        .map_err(|error| format!("{error:#}"))?;
        Ok(Completed::Page { cwd, page })
    }
}

pub(super) fn check(
    backend: Backend,
    cwd: &str,
    requested_socket: Option<&str>,
    configured_socket: Option<&std::path::Path>,
    cancel: &LaunchCancellation,
    deadline: Instant,
) -> Result<String, String> {
    cancel.check().map_err(|error| error.to_string())?;
    if Instant::now() >= deadline {
        return Err("saved-session request exceeded its deadline".into());
    }
    let path = PathBuf::from(cwd);
    if cwd.len() > 16384 || !path.is_absolute() {
        return Err("session cwd must be a bounded absolute directory".into());
    }
    let path = path
        .canonicalize()
        .map_err(|error| format!("session cwd: {error}"))?;
    if !path.is_dir() || path.to_str().is_none() {
        return Err("session cwd must be a UTF-8 directory".into());
    }
    validate_endpoint(backend, requested_socket, configured_socket)
        .map_err(|error| error.to_string())?;
    cancel.check().map_err(|error| error.to_string())?;
    if Instant::now() >= deadline {
        return Err("saved-session request exceeded its deadline".into());
    }
    Ok(path.to_str().expect("checked UTF-8").to_owned())
}
