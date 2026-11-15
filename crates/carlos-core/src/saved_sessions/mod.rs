//! Read bounded saved-session pages off the daemon manager without opening conversations.

mod backends;
mod window;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{ensure, Result};
use carlos_proto::session::Backend;

use crate::launch_cancel::LaunchCancellation;
use crate::session::models::ThreadSummary;

use self::window::Window;
pub use self::window::{PageAnchor, PageDirection};

/// The daemon retains only this many native choices for one connected picker.
pub const PAGE_ROWS: usize = 32;

/// Native addresses stay in core and the daemon; socket responses use unrelated opaque choices.
pub struct SavedPage {
    pub rows: Vec<ThreadSummary>,
    pub has_newer: bool,
    pub has_older: bool,
}

/// Describe one bounded read without exposing arbitrary native methods or parameters.
pub struct SavedPageRequest {
    pub backend: Backend,
    pub cwd: PathBuf,
    pub codex_socket: Option<PathBuf>,
    pub direction: PageDirection,
    /// Grok's continue command propagates native-list failures instead of scanning local files.
    pub allow_grok_fallback: bool,
}

/// Read pages into a fixed-size sorted window and stop the temporary reader before returning.
pub fn read_page(request: SavedPageRequest, cancel: &LaunchCancellation) -> Result<SavedPage> {
    cancel.check()?;
    ensure!(request.cwd.is_absolute(), "session cwd must be absolute");
    let cwd = request.cwd.canonicalize()?;
    ensure!(
        cwd.is_dir() && cwd.to_str().is_some(),
        "invalid session cwd"
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut window = Window::new(request.direction.clone());
    backends::visit(&request, &cwd, cancel, deadline, &mut |row| {
        let updated_at = row.updated_at;
        window.push(row);
        window.is_complete_before(updated_at)
    })?;
    cancel.check()?;
    Ok(window.finish())
}

pub(crate) fn remaining(cancel: &LaunchCancellation, deadline: Instant) -> Result<Duration> {
    cancel.check()?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    ensure!(
        !remaining.is_zero(),
        "saved-session read exceeded its deadline"
    );
    Ok(remaining.min(Duration::from_secs(20)))
}

#[cfg(test)]
mod tests;

#[cfg(all(test, unix))]
mod native_tests;
