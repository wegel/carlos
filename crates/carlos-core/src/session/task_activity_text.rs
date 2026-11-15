//! Render only the existing bounded child-panel scalar without copying complete native bodies.

use std::io::{self, Write};

use serde_json::Value;

/// A clipped display scalar remains distinct from the complete native record retained by its reader.
pub struct ActivityText {
    pub text: String,
    pub complete: bool,
}

/// Preserve the legacy child panel's text, command and fallback formatting within a byte bound.
pub fn display_item(item: &Value, limit: usize) -> ActivityText {
    let mut output = DisplayBytes {
        bytes: Vec::new(),
        limit,
        clipped: false,
    };
    let _ = match item["type"].as_str() {
        Some("agentMessage" | "plan") => output.value(&item["text"]),
        Some("commandExecution") => command(&mut output, item),
        _ => serde_json::to_writer_pretty(&mut output, item).map_err(io::Error::other),
    };
    if let Err(error) = std::str::from_utf8(&output.bytes) {
        output.bytes.truncate(error.valid_up_to());
    }
    ActivityText {
        text: String::from_utf8(output.bytes).expect("native display scalar UTF-8"),
        complete: !output.clipped,
    }
}

struct DisplayBytes {
    bytes: Vec<u8>,
    limit: usize,
    clipped: bool,
}

impl Write for DisplayBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let remaining = self.limit.saturating_sub(self.bytes.len());
        self.bytes
            .extend_from_slice(&bytes[..remaining.min(bytes.len())]);
        if bytes.len() > remaining {
            self.clipped = true;
            return Err(io::Error::other(
                "child display scalar reached its byte bound",
            ));
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl DisplayBytes {
    fn value(&mut self, value: &Value) -> io::Result<()> {
        match value {
            Value::Null => self.write_all(b"unavailable"),
            Value::String(text) => self.write_all(text.as_bytes()),
            _ => write!(self, "{value}"),
        }
    }
}

fn command(output: &mut DisplayBytes, item: &Value) -> io::Result<()> {
    output.value(&item["command"])?;
    output.write_all(b" [")?;
    output.value(&item["status"])?;
    output.write_all(b"]\n")?;
    output.value(&item["aggregatedOutput"])
}
