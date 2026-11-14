//! Bound native input previews before cloning large persisted text or attachment objects.

use std::io::{self, Write};

use serde_json::Value;

/// Preserve the terminal's text-or-JSON preview without copying the full native input.
pub(super) fn preview(input: &Value) -> (String, bool) {
    let mut output = Preview {
        text: String::new(),
        remaining: 1024,
        complete: true,
    };
    for (index, part) in input.as_array().into_iter().flatten().enumerate() {
        if index > 0 && output.write_all(b"\n").is_err() {
            break;
        }
        let result = if let Some(text) = part["text"].as_str() {
            output.write_all(text.as_bytes())
        } else {
            serde_json::to_writer(&mut output, part).map_err(io::Error::other)
        };
        if result.is_err() {
            break;
        }
    }
    (output.text, output.complete)
}

struct Preview {
    text: String,
    remaining: usize,
    complete: bool,
}

impl Write for Preview {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        // serde_json writes whole UTF-8 source slices and ASCII JSON escapes.
        let text = std::str::from_utf8(bytes).map_err(io::Error::other)?;
        let end = text
            .char_indices()
            .nth(self.remaining)
            .map_or(text.len(), |(offset, _)| offset);
        self.remaining -= text[..end].chars().count();
        self.text.push_str(&text[..end]);
        if end < text.len() {
            self.complete = false;
            return Err(io::Error::other("preview is complete"));
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
