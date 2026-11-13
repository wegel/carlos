//! Probe owned CLI features with a bounded read before choosing compatible process flags.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const LIMIT: u64 = 64 * 1024;

/// Keep supported live-setting controls separate from a default-off goal feature.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct OwnedFeatures {
    pub(crate) live_settings: bool,
    pub(crate) enable_goals: bool,
}

/// Choose only process-local flags advertised by the installed native CLI.
pub(crate) fn owned_features() -> OwnedFeatures {
    let mut command = Command::new("codex");
    command.args(["features", "list"]);
    probe(command, Duration::from_secs(2))
}

fn probe(mut command: Command, timeout: Duration) -> OwnedFeatures {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let Ok(mut child) = command.spawn() else {
        return OwnedFeatures::default();
    };
    let Some(stdout) = child.stdout.take() else {
        stop(&mut child);
        return OwnedFeatures::default();
    };
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = sender.send(result);
    });
    let deadline = Instant::now() + timeout;
    let Some(output) = receiver.recv_timeout(timeout).ok().and_then(Result::ok) else {
        stop(&mut child);
        return OwnedFeatures::default();
    };
    if output.len() > LIMIT as usize {
        stop(&mut child);
        return OwnedFeatures::default();
    }
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            _ if Instant::now() >= deadline => break None,
            _ => thread::sleep(Duration::from_millis(5)),
        }
    };
    let supported = if status.is_some_and(|status| status.success()) {
        advertises(&output)
    } else {
        OwnedFeatures::default()
    };
    if status.is_none() {
        stop(&mut child);
    }
    supported
}

fn advertises(output: &[u8]) -> OwnedFeatures {
    let mut features = OwnedFeatures::default();
    if let Ok(text) = std::str::from_utf8(output) {
        for line in text.lines() {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            if !matches!(fields.last(), Some(&"true" | &"false"))
                || fields.contains(&"removed")
                || fields.contains(&"deprecated")
            {
                continue;
            }
            match fields.first() {
                Some(&"step_model_switching") => features.live_settings = true,
                Some(&"goals") if fields.last() == Some(&"false") => features.enable_goals = true,
                _ => {}
            }
        }
    }
    features
}

fn stop(child: &mut std::process::Child) {
    // The caller has not reaped this probe, so its PID/group cannot have been reused.
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
#[path = "tests/protocol_capability_tests.rs"]
mod tests;
