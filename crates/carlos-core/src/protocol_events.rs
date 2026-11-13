//! Keep one charged Codex event stream across connection generations.

use std::sync::{mpsc, Arc, Mutex};

use anyhow::{ensure, Result};
use serde_json::{json, Value};

const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_FRAMES: usize = 4096;
const EMERGENCY_BYTES: usize = 64 * 1024;

#[derive(Clone)]
/// Bounds frames across readers, reconnects and the UI forwarding queue.
pub(crate) struct EventQueue {
    sender: mpsc::Sender<String>,
    usage: Arc<Mutex<(usize, usize)>>,
    generation: u64,
}

impl EventQueue {
    /// Create the stable stream; standalone pipe tests keep unwrapped native frames.
    pub(crate) fn new() -> (Self, mpsc::Receiver<String>) {
        let (sender, receiver) = mpsc::channel();
        (
            Self {
                sender,
                usage: Arc::new(Mutex::new((0, 0))),
                generation: 0,
            },
            receiver,
        )
    }

    /// Stamp this connection without replacing its shared queue or acknowledgments.
    pub(crate) fn for_generation(&self, generation: u64) -> Self {
        Self {
            generation,
            ..self.clone()
        }
    }

    /// Charge the complete forwarded envelope until the UI acknowledges it.
    pub(crate) fn send(&self, line: String) -> Result<()> {
        self.enqueue(self.wrap(line), false)
    }

    /// Charge a bounded emergency diagnostic even when ordinary frames fill the queue.
    pub(crate) fn terminal(&self, line: String) {
        let line = self.wrap(line);
        if line.len() <= EMERGENCY_BYTES {
            let _ = self.enqueue(line, true);
        }
    }

    fn wrap(&self, line: String) -> String {
        if self.generation == 0 {
            return line;
        }
        let event: Value = serde_json::from_str(&line).expect("parsed Codex event");
        json!({"method":"carlos/codex/frame", "params":{
            "generation":self.generation,"event":event}})
        .to_string()
    }

    fn enqueue(&self, line: String, terminal: bool) -> Result<()> {
        let mut usage = self.usage.lock().expect("Codex event budget poisoned");
        let (bytes, frames) = if terminal {
            (MAX_BYTES + EMERGENCY_BYTES, MAX_FRAMES + 1)
        } else {
            (MAX_BYTES, MAX_FRAMES)
        };
        ensure!(
            usage.0 + line.len() <= bytes && usage.1 < frames,
            "Codex UI event backlog exceeded 16 MiB or 4096 frames"
        );
        let length = line.len();
        usage.0 += length;
        usage.1 += 1;
        if self.sender.send(line).is_err() {
            usage.0 -= length;
            usage.1 -= 1;
            anyhow::bail!("Codex UI event receiver closed");
        }
        Ok(())
    }

    /// Release only the actual forwarded bytes, including reserved terminal notices.
    pub(crate) fn handled(&self, length: usize) {
        if let Ok(mut usage) = self.usage.lock() {
            usage.0 = usage.0.saturating_sub(length);
            usage.1 = usage.1.saturating_sub(1);
        }
    }
}

/// Keep the useful failure reason on screen and within the emergency frame.
pub(crate) fn bounded_diagnostic(message: &str) -> String {
    let mut output = String::with_capacity(512);
    let mut space = false;
    for character in message.chars() {
        let character = if character.is_whitespace() {
            ' '
        } else if character.is_control() {
            '\u{fffd}'
        } else {
            character
        };
        if character == ' ' && space {
            continue;
        }
        if output.len() + character.len_utf8() > 512 {
            output.push_str(" [details truncated]");
            break;
        }
        output.push(character);
        space = character == ' ';
    }
    output
}
