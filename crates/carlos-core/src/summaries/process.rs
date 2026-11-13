//! Run a bounded child process without waiting on its pipes from the UI thread.

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use super::Control;
use anyhow::{bail, Context, Result};

const OUTPUT_LIMIT: u64 = 524_288;

struct ChildGuard(Child, Arc<Control>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self
            .1
            .pid
            .compare_exchange(self.0.id(), 0, Ordering::Relaxed, Ordering::Relaxed);
        #[cfg(unix)]
        // The child created its own group; never signal Carlos's process group.
        unsafe {
            libc::kill(-(self.0.id() as i32), libc::SIGKILL);
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Bound wall time and output, and interrupt promptly when Carlos disables/exits.
pub(super) fn run(
    command: &mut Command,
    input: String,
    timeout: Duration,
    control: &Arc<Control>,
    epoch: u64,
) -> Result<String> {
    let started = Instant::now();
    let (mut child, rx) = start(command, input, control, epoch)?;
    loop {
        if control.stopped.load(Ordering::Relaxed)
            || (!control.enabled.load(Ordering::Relaxed)
                || control.epoch.load(Ordering::Relaxed) != epoch)
        {
            bail!("Summary worker cancelled");
        }
        if started.elapsed() >= timeout {
            bail!("Summary worker timed out");
        }
        if let Ok(result) = rx.try_recv() {
            let bytes = result?;
            if bytes.len() as u64 > OUTPUT_LIMIT {
                bail!("Summary output exceeded limit");
            }
            let text = String::from_utf8(bytes).context("Summary output is not UTF-8")?;
            loop {
                if let Some(status) = child.0.try_wait()? {
                    if !status.success() {
                        bail!("Summary executable exited with {status}");
                    }
                    return Ok(text);
                }
                if started.elapsed() >= timeout
                    || control.stopped.load(Ordering::Relaxed)
                    || (!control.enabled.load(Ordering::Relaxed)
                        || control.epoch.load(Ordering::Relaxed) != epoch)
                {
                    bail!("Summary worker cancelled or timed out");
                }
                thread::sleep(Duration::from_millis(20));
            }
        }
        if let Some(status) = child.0.try_wait()? {
            if !status.success() {
                bail!("Summary executable exited with {status}");
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn start(
    command: &mut Command,
    input: String,
    control: &Arc<Control>,
    epoch: u64,
) -> Result<(ChildGuard, mpsc::Receiver<std::io::Result<Vec<u8>>>)> {
    if control.stopped.load(Ordering::Relaxed)
        || (!control.enabled.load(Ordering::Relaxed)
            || control.epoch.load(Ordering::Relaxed) != epoch)
    {
        bail!("Summary worker cancelled");
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = ChildGuard(
        command.spawn().context("start summary executable")?,
        control.clone(),
    );
    control.pid.store(child.0.id(), Ordering::Relaxed);
    let mut stdin = child.0.stdin.take().context("summary stdin unavailable")?;
    thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });
    let stdout = child
        .0
        .stdout
        .take()
        .context("summary stdout unavailable")?;
    let (tx, rx) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(OUTPUT_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = tx.send(result);
    });
    Ok((child, rx))
}
