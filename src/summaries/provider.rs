//! Keep model commands behind a replaceable adapter with bounded native processes.

mod codex;

use std::process::Command;
use std::sync::Arc;

use anyhow::{bail, Result};
use serde_json::json;

use super::config::Config;
use super::{process, Control, Label, Request};

pub(super) const PROMPT: &str = include_str!("prompt.txt");

/// Replace model/runtime choices without changing transcript or database code.
pub(super) trait Summarizer: Send {
    fn summarize(&mut self, request: &Request, control: &Arc<Control>, epoch: u64)
        -> Result<Label>;
}

pub(super) fn create(config: &Config) -> Box<dyn Summarizer> {
    if config.codex {
        Box::new(codex::Codex(config.clone()))
    } else {
        Box::new(External(config.clone()))
    }
}

struct External(Config);

impl Summarizer for External {
    fn summarize(
        &mut self,
        request: &Request,
        control: &Arc<Control>,
        epoch: u64,
    ) -> Result<Label> {
        let config = &self.0;
        let mut command = Command::new(&config.executable);
        command.args(&config.arguments).current_dir(&request.cwd);
        let input =
            json!({"command":request.command,"cwd":request.cwd,"instructions":PROMPT}).to_string();
        let text = process::run(&mut command, input, config.timeout, control, epoch)?;
        Ok(Label {
            text: validate(&text)?,
            sources: Vec::new(),
        })
    }
}

/// Reject multiline/control output and keep labels short enough to inspect.
pub(super) fn validate(text: &str) -> Result<String> {
    let text = text.trim();
    if text.is_empty()
        || text.chars().count() > 180
        || text.split_whitespace().count() > 24
        || text.chars().any(char::is_control)
        || text.starts_with("```")
    {
        bail!("Summary worker returned an invalid label");
    }
    Ok(text.trim_matches('`').to_owned())
}
