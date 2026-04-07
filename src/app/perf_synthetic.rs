//! Synthetic performance data generation: deterministic RNG, message builders, and word lists.

use super::models::{Message, MessageKind, Role};
use super::transcript_render::format_read_summary_with_count;

// --- Core Types ---
const SYNTHETIC_USER: &str = "perf-user";
const SYNTHETIC_HOST: &str = "perfbox.local";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SyntheticPerfSpec {
    pub(super) seed: u64,
    pub(super) turns: usize,
    pub(super) tool_output_lines: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SyntheticRng(u64);

// --- RNG Helpers ---
impl SyntheticRng {
    pub(super) fn new(seed: u64) -> Self {
        Self(if seed == 0 { 1 } else { seed })
    }

    pub(super) fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub(super) fn range(&mut self, upper: usize) -> usize {
        if upper <= 1 {
            0
        } else {
            (self.next_u64() as usize) % upper
        }
    }

    pub(super) fn choose<'a>(&mut self, items: &'a [&'static str]) -> &'a str {
        items[self.range(items.len())]
    }
}

// --- Message Builders ---
pub(super) fn build_synthetic_perf_messages(spec: SyntheticPerfSpec) -> Vec<Message> {
    let mut rng = SyntheticRng::new(spec.seed);
    let mut messages = Vec::with_capacity(spec.turns.saturating_mul(6));

    for turn in 0..spec.turns {
        messages.push(Message {
            role: Role::User,
            text: synthetic_user_message(turn, &mut rng),
            kind: MessageKind::Plain,
            file_path: None,
        });
        messages.push(Message {
            role: Role::Commentary,
            text: synthetic_commentary_message(turn, &mut rng),
            kind: MessageKind::Plain,
            file_path: None,
        });
        messages.push(Message {
            role: Role::Reasoning,
            text: synthetic_reasoning_message(turn, &mut rng),
            kind: MessageKind::Plain,
            file_path: None,
        });
        messages.push(Message {
            role: Role::ToolCall,
            text: synthetic_tool_call(turn, &mut rng),
            kind: MessageKind::Plain,
            file_path: None,
        });
        messages.push(synthetic_tool_output(turn, spec, &mut rng));
        messages.push(Message {
            role: Role::Assistant,
            text: synthetic_assistant_message(turn, &mut rng),
            kind: MessageKind::Plain,
            file_path: None,
        });
    }

    messages
}

// --- Turn Content ---
fn synthetic_user_message(turn: usize, rng: &mut SyntheticRng) -> String {
    let intro = synthetic_paragraph(rng, 2, 9, 14);
    if turn % 7 == 0 {
        format!(
            "{intro}\n\n```rust\n{}\n```",
            synthetic_code_block(turn, rng)
        )
    } else {
        intro
    }
}

fn synthetic_commentary_message(turn: usize, rng: &mut SyntheticRng) -> String {
    format!(
        "{}\n{}",
        synthetic_heading(rng),
        synthetic_paragraph(rng, 1 + (turn % 2), 10, 16)
    )
}

fn synthetic_reasoning_message(turn: usize, rng: &mut SyntheticRng) -> String {
    let count = 2 + (turn % 4);
    let mut lines = Vec::with_capacity(count);
    for _ in 0..count {
        lines.push(format!("**{}**", synthetic_heading(rng)));
    }
    lines.join("\n")
}

fn synthetic_tool_call(turn: usize, rng: &mut SyntheticRng) -> String {
    match turn % 4 {
        0 | 1 => {
            let path = format!(
                "src/module_{:02}/file_{:03}.rs",
                (turn / 6) % 48,
                (turn / 2) % 192
            );
            format_read_summary_with_count(&path, 1)
        }
        2 => format!(
            "remote exec on {}@{}\n$ {}",
            SYNTHETIC_USER,
            SYNTHETIC_HOST,
            synthetic_shell_pipeline(turn, rng)
        ),
        _ => format!(
            "run `cargo test -p crate_{:02} {} -- --nocapture`",
            turn % 19,
            synthetic_identifier(rng)
        ),
    }
}

fn synthetic_tool_output(turn: usize, spec: SyntheticPerfSpec, rng: &mut SyntheticRng) -> Message {
    if turn % 9 == 0 {
        return Message {
            role: Role::ToolOutput,
            kind: MessageKind::Diff,
            file_path: Some(format!(
                "src/module_{:02}/perf_case_{:03}.rs",
                turn % 32,
                turn
            )),
            text: synthetic_diff(turn, rng),
        };
    }

    let mut lines = Vec::with_capacity(spec.tool_output_lines.max(1));
    for idx in 0..spec.tool_output_lines.max(1) {
        let level = match idx % 3 {
            0 => "\u{1b}[32mINFO\u{1b}[0m",
            1 => "\u{1b}[33mWARN\u{1b}[0m",
            _ => "\u{1b}[36mDEBUG\u{1b}[0m",
        };
        lines.push(format!(
            "{level} turn={turn:05} line={idx:03} {} {} {}",
            synthetic_identifier(rng),
            synthetic_heading(rng).to_ascii_lowercase(),
            synthetic_paragraph(rng, 1, 8, 12)
        ));
    }

    Message {
        role: Role::ToolOutput,
        kind: MessageKind::Plain,
        file_path: None,
        text: lines.join("\n"),
    }
}

fn synthetic_assistant_message(turn: usize, rng: &mut SyntheticRng) -> String {
    let mut sections = vec![synthetic_paragraph(rng, 2, 10, 16)];
    if turn % 5 == 0 {
        sections.push(format!(
            "- {}\n- {}\n- {}",
            synthetic_sentence(rng, 8, 12),
            synthetic_sentence(rng, 8, 12),
            synthetic_sentence(rng, 8, 12)
        ));
    }
    sections.join("\n\n")
}

// --- Content Generators ---
fn synthetic_code_block(turn: usize, rng: &mut SyntheticRng) -> String {
    let value = 10 + rng.range(90);
    format!(
        "fn case_{turn:04}() {{\n    let {} = {value};\n    println!(\"{{}}\", {});\n}}",
        synthetic_identifier(rng),
        synthetic_identifier(rng)
    )
}

fn synthetic_diff(turn: usize, rng: &mut SyntheticRng) -> String {
    format!(
        "@@ -{old_line},3 +{new_line},5 @@\n-{}\n-{}\n+{}\n+{}\n+{}",
        synthetic_sentence(rng, 6, 9),
        synthetic_sentence(rng, 6, 9),
        synthetic_sentence(rng, 7, 11),
        synthetic_sentence(rng, 7, 11),
        synthetic_sentence(rng, 7, 11),
        old_line = 10 + (turn % 70),
        new_line = 10 + (turn % 70)
    )
}

fn synthetic_shell_pipeline(turn: usize, rng: &mut SyntheticRng) -> String {
    format!(
        "set -euo pipefail; jq -r '.items[] | .name' {} | sed -n '1,40p' | awk 'NF'",
        synthetic_remote_path(turn, rng)
    )
}

fn synthetic_remote_path(turn: usize, rng: &mut SyntheticRng) -> String {
    format!(
        "/srv/{}/jobs/{:04}/{}.json",
        synthetic_identifier(rng),
        turn % 10_000,
        synthetic_identifier(rng)
    )
}

fn synthetic_heading(rng: &mut SyntheticRng) -> String {
    let mut words = vec![
        rng.choose(&HEADING_VERBS).to_string(),
        rng.choose(&HEADING_OBJECTS).to_string(),
        rng.choose(&HEADING_OBJECTS).to_string(),
    ];
    for word in &mut words {
        if let Some(first) = word.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        break;
    }
    words.join(" ")
}

fn synthetic_paragraph(
    rng: &mut SyntheticRng,
    sentences: usize,
    min_words: usize,
    max_words: usize,
) -> String {
    (0..sentences)
        .map(|_| synthetic_sentence(rng, min_words, max_words))
        .collect::<Vec<_>>()
        .join(" ")
}

fn synthetic_sentence(rng: &mut SyntheticRng, min_words: usize, max_words: usize) -> String {
    let span = max_words.saturating_sub(min_words);
    let words = min_words + rng.range(span.max(1) + 1);
    let mut out = String::new();
    for idx in 0..words {
        let word = if idx == 0 {
            rng.choose(&SYNTHETIC_WORDS).to_ascii_uppercase()
        } else {
            rng.choose(&SYNTHETIC_WORDS).to_string()
        };
        if idx > 0 {
            out.push(' ');
        }
        out.push_str(&word);
        if idx + 1 == words / 2 && words > 10 && rng.range(3) == 0 {
            out.push(',');
        }
    }
    out.push('.');
    out
}

fn synthetic_identifier(rng: &mut SyntheticRng) -> String {
    format!("{}_{}", rng.choose(&SYNTHETIC_WORDS), 10 + rng.range(990))
}

// --- Word Lists ---
const SYNTHETIC_WORDS: [&str; 40] = [
    "atlas", "buffer", "cache", "delta", "engine", "frame", "graph", "handle", "index", "journal",
    "kernel", "layout", "marker", "native", "offset", "packet", "queue", "render", "sample",
    "token", "update", "vector", "window", "yield", "bridge", "cursor", "driver", "event",
    "format", "guard", "history", "input", "ledger", "metric", "notice", "output", "parser",
    "reader", "signal", "worker",
];

const HEADING_VERBS: [&str; 12] = [
    "analyzing",
    "checking",
    "profiling",
    "reviewing",
    "comparing",
    "tracing",
    "stabilizing",
    "measuring",
    "inspecting",
    "sampling",
    "tracking",
    "validating",
];

const HEADING_OBJECTS: [&str; 16] = [
    "viewport",
    "render",
    "history",
    "selection",
    "input",
    "layout",
    "scroll",
    "diff",
    "session",
    "cursor",
    "repaint",
    "metrics",
    "tooling",
    "state",
    "wrapping",
    "output",
];
