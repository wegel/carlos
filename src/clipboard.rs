use std::env;
use std::io::{self, Write};
use std::process::{Command, Stdio};

use base64::Engine;

fn copy_via_program(argv: &[&str], text: &str) -> bool {
    let Some((cmd, args)) = argv.split_first() else {
        return false;
    };

    let mut child = match Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }

    child.wait().map(|s| s.success()).unwrap_or(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Osc52Wrap {
    None,
    Tmux,
    Screen,
}

pub(crate) fn detect_osc52_wrap(tmux: Option<&str>, term: Option<&str>) -> Osc52Wrap {
    if tmux.is_some_and(|v| !v.is_empty()) {
        return Osc52Wrap::Tmux;
    }
    if term.is_some_and(|t| t.contains("screen")) {
        return Osc52Wrap::Screen;
    }
    Osc52Wrap::None
}

pub(crate) fn osc52_base_sequence(target: &str, encoded: &str, use_st_terminator: bool) -> String {
    if use_st_terminator {
        format!("\x1b]52;{};{}\x1b\\", target, encoded)
    } else {
        format!("\x1b]52;{};{}\x07", target, encoded)
    }
}

fn wrap_osc52_sequence(seq: &str, wrap: Osc52Wrap) -> String {
    match wrap {
        Osc52Wrap::None => seq.to_string(),
        Osc52Wrap::Tmux => {
            // tmux passthrough requires DCS wrapper and escaping nested ESC bytes.
            let escaped = seq.replace('\x1b', "\x1b\x1b");
            format!("\x1bPtmux;{}\x1b\\", escaped)
        }
        Osc52Wrap::Screen => {
            // GNU screen passthrough wrapper.
            format!("\x1bP{}\x1b\\", seq)
        }
    }
}

pub(crate) fn osc52_sequences_for_env(
    encoded: &str,
    tmux: Option<&str>,
    term: Option<&str>,
) -> Vec<String> {
    let wrap = detect_osc52_wrap(tmux, term);
    let mut out = Vec::with_capacity(4);
    for target in ["c", "p"] {
        out.push(wrap_osc52_sequence(
            &osc52_base_sequence(target, encoded, false),
            wrap,
        ));
        out.push(wrap_osc52_sequence(
            &osc52_base_sequence(target, encoded, true),
            wrap,
        ));
    }
    out
}

fn copy_via_osc52(text: &str) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    let tmux = env::var("TMUX").ok();
    let term = env::var("TERM").ok();
    let sequences = osc52_sequences_for_env(&encoded, tmux.as_deref(), term.as_deref());

    let mut stdout = io::stdout();
    for seq in &sequences {
        let _ = stdout.write_all(seq.as_bytes());
    }
    let _ = stdout.flush();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClipboardBackend {
    Osc52,
    Program(&'static str),
    None,
}

pub(crate) fn is_ssh_session(
    ssh_tty: Option<&str>,
    ssh_connection: Option<&str>,
    ssh_client: Option<&str>,
) -> bool {
    ssh_tty.is_some_and(|v| !v.is_empty())
        || ssh_connection.is_some_and(|v| !v.is_empty())
        || ssh_client.is_some_and(|v| !v.is_empty())
}

pub(crate) fn try_copy_clipboard(text: &str) -> ClipboardBackend {
    if text.is_empty() {
        return ClipboardBackend::None;
    }

    if is_ssh_session(
        env::var("SSH_TTY").ok().as_deref(),
        env::var("SSH_CONNECTION").ok().as_deref(),
        env::var("SSH_CLIENT").ok().as_deref(),
    ) {
        copy_via_osc52(text);
        return ClipboardBackend::Osc52;
    }

    if copy_via_program(&["wl-copy"], text) {
        return ClipboardBackend::Program("wl-copy");
    }
    if copy_via_program(&["xclip", "-selection", "clipboard"], text) {
        return ClipboardBackend::Program("xclip");
    }
    if copy_via_program(&["xsel", "--clipboard", "--input"], text) {
        return ClipboardBackend::Program("xsel");
    }
    if copy_via_program(&["pbcopy"], text) {
        return ClipboardBackend::Program("pbcopy");
    }
    copy_via_osc52(text);
    ClipboardBackend::Osc52
}

pub(crate) fn clipboard_backend_label(backend: ClipboardBackend) -> &'static str {
    match backend {
        ClipboardBackend::Osc52 => "osc52",
        ClipboardBackend::Program(name) => name,
        ClipboardBackend::None => "none",
    }
}
