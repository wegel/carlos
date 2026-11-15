//! Capture only explicit terminal environment choices for new native processes.

use anyhow::{Context, Result};
use carlos_proto::session::Backend;
use carlos_proto::startup::{NativeLaunchOptions, ReasoningSummary};

use crate::app::cli::{Backend as CliBackend, CliOptions};

pub(super) enum Intent {
    New,
    Resume(String),
    Continue,
    Picker,
}

pub(super) struct Options {
    pub backend: Backend,
    pub cwd: String,
    pub launch: NativeLaunchOptions,
    pub tool_labels: Option<bool>,
    pub intent: Intent,
}

impl Options {
    pub(super) fn capture(options: &CliOptions) -> Result<Self> {
        let backend = match options.backend {
            CliBackend::Codex => Backend::Codex,
            CliBackend::Claude => Backend::Claude,
            CliBackend::Grok => Backend::Grok,
        };
        let intent = if options.mode_continue {
            Intent::Continue
        } else if let Some(address) = &options.resume_id {
            Intent::Resume(address.clone())
        } else if options.mode_resume {
            Intent::Picker
        } else {
            Intent::New
        };
        let cwd = std::env::current_dir().context("read session working directory")?;
        let cwd = cwd
            .to_str()
            .context("session working directory is not UTF-8")?
            .to_owned();
        let codex_socket = options
            .codex_socket
            .as_ref()
            .map(|path| {
                path.to_str()
                    .context("Codex endpoint path is not UTF-8")
                    .map(str::to_owned)
            })
            .transpose()?;
        let reasoning_summary = if backend == Backend::Codex {
            std::env::var("CARLOS_REASONING_SUMMARY")
                .ok()
                .map(|value| summary(&value))
        } else {
            None
        };
        let claude_context_window = if backend == Backend::Claude {
            std::env::var("CLAUDE_CODE_AUTO_COMPACT_WINDOW").ok()
        } else {
            None
        };
        Ok(Self {
            backend,
            cwd,
            intent,
            launch: NativeLaunchOptions {
                codex_socket,
                reasoning_summary,
                claude_context_window,
            },
            tool_labels: std::env::var("CARLOS_TOOL_SUMMARIES")
                .ok()
                .map(|value| matches!(value.as_str(), "on" | "true" | "1")),
        })
    }
}

fn summary(value: &str) -> ReasoningSummary {
    match value.trim() {
        "concise" => ReasoningSummary::Concise,
        "detailed" => ReasoningSummary::Detailed,
        "none" => ReasoningSummary::None,
        _ => ReasoningSummary::Auto,
    }
}
