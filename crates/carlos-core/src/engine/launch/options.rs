//! Check explicit terminal endpoint choices and keep first-launch environment values private.

use std::path::{Path, PathBuf};

use carlos_proto::session::Backend;
use carlos_proto::startup::NativeLaunchOptions;

use super::LaunchError;

/// Keep daemon endpoint ownership separate from optional choices carried by a new terminal launch.
#[derive(Default)]
pub struct LaunchOptions {
    pub codex_socket: Option<PathBuf>,
    pub tool_labels: Option<bool>,
    pub native: NativeLaunchOptions,
}

/// Compare actual filesystem endpoints off the manager; a missing constraint accepts its default.
pub fn validate_endpoint(
    backend: Backend,
    requested: Option<&str>,
    configured: Option<&Path>,
) -> Result<(), LaunchError> {
    let Some(requested) = requested else {
        return Ok(());
    };
    if backend != Backend::Codex {
        return Err(LaunchError::Invalid(
            "a Codex endpoint is valid only for Codex".into(),
        ));
    }
    let Some(configured) = configured else {
        return Err(LaunchError::Invalid(
            "this daemon owns its Codex process; the explicit Codex socket does not match".into(),
        ));
    };
    let requested = Path::new(requested);
    if !requested.is_absolute() || requested.as_os_str().len() > 16384 {
        return Err(LaunchError::Invalid(
            "the explicit Codex socket must be a bounded absolute path".into(),
        ));
    }
    let canonical = |path: &Path| {
        path.canonicalize().map_err(|error| {
            LaunchError::Invalid(format!("Codex endpoint {}: {error}", path.display()))
        })
    };
    if canonical(requested)? != canonical(configured)? {
        return Err(LaunchError::Invalid(
            "the explicit Codex socket differs from this daemon's configured endpoint".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_options(
    backend: Backend,
    options: &LaunchOptions,
) -> Result<(), LaunchError> {
    validate_endpoint(
        backend,
        options.native.codex_socket.as_deref(),
        options.codex_socket.as_deref(),
    )?;
    if options.native.reasoning_summary.is_some() && backend != Backend::Codex {
        return Err(LaunchError::Invalid(
            "a reasoning summary choice is valid only for Codex".into(),
        ));
    }
    if options.native.claude_context_window.is_some() && backend != Backend::Claude {
        return Err(LaunchError::Invalid(
            "a Claude context environment choice is valid only for Claude".into(),
        ));
    }
    Ok(())
}
