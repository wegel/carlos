//! Retire an owned process group before reaping its leader can release the group's identity.

use std::io;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus};

/// Keep the only child-reaping handle beside its dedicated process-group lifetime.
pub(crate) struct OwnedChild {
    pub stdin: Option<ChildStdin>,
    pub stdout: Option<ChildStdout>,
    pub stderr: Option<ChildStderr>,
    child: Child,
    status: Option<ExitStatus>,
    killed: bool,
}

impl OwnedChild {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command.spawn()?;
        Ok(Self {
            stdin: child.stdin.take(),
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
            child,
            status: None,
            killed: false,
        })
    }

    /// Terminate this family gently while its unreaped leader still reserves its identity.
    pub fn terminate(&mut self) {
        if self.status.is_some() || self.killed {
            return;
        }
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGTERM);
        }
        #[cfg(not(unix))]
        {
            let _ = self.child.kill();
        }
    }

    /// Close the whole owned family exactly once; a cached reaped PID is never signalled.
    pub fn kill(&mut self) -> io::Result<()> {
        if self.killed || self.status.is_some() {
            return Ok(());
        }
        self.killed = true;
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGKILL);
        }
        self.child.kill()
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if let Some(status) = self.status {
            return Ok(Some(status));
        }
        #[cfg(unix)]
        {
            // WNOWAIT observes exit without releasing the leader's PID. Kill remaining
            // descendants before Child::try_wait can reap that leader and release the ID.
            let finished = exited(self.child.id()).inspect_err(|error| {
                if error.raw_os_error() == Some(libc::ECHILD) {
                    // An external reaper released this PID; never signal it on later cleanup.
                    self.killed = true;
                }
            })?;
            if !finished {
                return Ok(None);
            }
            let _ = self.kill();
        }
        self.status = self.child.try_wait()?;
        Ok(self.status)
    }

    /// Finish explicit cleanup, including descendants that kept the leader's pipes open.
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        if let Some(status) = self.status {
            return Ok(status);
        }
        let _ = self.kill();
        let status = self.child.wait()?;
        self.status = Some(status);
        Ok(status)
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.wait();
    }
}

#[cfg(unix)]
fn exited(pid: u32) -> io::Result<bool> {
    let mut status = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
    let result = unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            &mut status,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { status.si_pid() } != 0)
}

#[cfg(all(test, target_os = "linux"))]
#[path = "tests/owned_child_tests.rs"]
mod tests;
