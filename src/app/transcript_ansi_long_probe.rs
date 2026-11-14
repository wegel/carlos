//! Ignored isolated-process measurements for the saved long-line index.

#[path = "transcript_ansi_components_probe.rs"]
mod components;

use std::env;
use std::fs;
use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use carlos_core::session::models::Role;
use serde_json::json;

use super::plain::PlainPages;
use super::AnsiPages;

enum Pages {
    Ansi(AnsiPages),
    Plain(PlainPages),
}

impl Pages {
    fn new(source: String, plain: bool) -> Self {
        let source = Arc::new(source);
        if plain {
            Self::Plain(PlainPages::new(source, Role::ToolOutput, 28).expect("plain index"))
        } else {
            Self::Ansi(AnsiPages::new(source, Role::ToolOutput, 28).expect("ANSI index"))
        }
    }

    fn rows(&self) -> usize {
        match self {
            Self::Ansi(pages) => pages.rows(),
            Self::Plain(pages) => pages.rows(),
        }
    }

    fn window(&self, start: usize) -> usize {
        match self {
            Self::Ansi(pages) => black_box(pages.window(start, 50).expect("viewport")).len(),
            Self::Plain(pages) => black_box(pages.window(start, 50)).len(),
        }
    }

    fn append(&mut self, text: &str) {
        match self {
            Self::Ansi(pages) => pages.append(text).expect("append"),
            Self::Plain(pages) => pages.append(text),
        }
    }

    fn resize(self) -> Self {
        match self {
            Self::Ansi(pages) => Self::Ansi(pages.resize(80).expect("resize")),
            Self::Plain(pages) => Self::Plain(pages.resize(80).expect("resize")),
        }
    }

    fn index_capacity_bytes(&self) -> usize {
        match self {
            Self::Ansi(pages) => pages.index_capacity_bytes(),
            Self::Plain(pages) => pages.index_capacity_bytes(),
        }
    }

    fn original_text(&self) -> &str {
        match self {
            Self::Ansi(pages) => pages.original_text(),
            Self::Plain(pages) => pages.original_text(),
        }
    }
}

#[test]
#[ignore = "run a release executable in its own process with CARLOS_ANSI_INDEX_CASE"]
fn measure_saved_long_line_ranges() {
    let case = env::var("CARLOS_ANSI_INDEX_CASE").expect("explicit probe case");
    let mib = env::var("CARLOS_ANSI_INDEX_MIB").unwrap_or_else(|_| "1".into());
    let source = source(&case, mib.parse::<usize>().expect("MiB"));
    let bytes = source.len();
    let mut original = blake3::Hasher::new();
    original.update(source.as_bytes());
    let digest = original.finalize().to_hex().to_string();
    let initial = Instant::now();
    let mut pages = Pages::new(source, case.starts_with("plain:"));
    let initial_ms = initial.elapsed().as_secs_f64() * 1000.0;
    let index_bytes_28 = pages.index_capacity_bytes();
    let rows_28 = pages.rows();
    let retained = status();
    let (mut windows, rows) = window_samples(&pages);
    let addition = if case == "literal" {
        "MORE9"
    } else {
        " next, 界\t🙂"
    };
    original.update(addition.as_bytes());
    let append = Instant::now();
    pages.append(addition);
    let append_ms = append.elapsed().as_secs_f64() * 1000.0;
    let resize = Instant::now();
    pages = pages.resize();
    let resize_ms = resize.elapsed().as_secs_f64() * 1000.0;
    let original_blake3_after = blake3::hash(pages.original_text().as_bytes())
        .to_hex()
        .to_string();
    assert_eq!(
        original_blake3_after,
        original.finalize().to_hex().to_string()
    );
    windows.sort_by(f64::total_cmp);
    println!(
        "ANSI_INDEX {}",
        json!({
            "case": case, "mib": mib, "bytes": bytes, "blake3": digest,
            "initial_ms": initial_ms, "append_ms": append_ms, "resize_ms": resize_ms,
            "index_bytes_80": pages.index_capacity_bytes(), "rows_80": pages.rows(),
            "index_bytes_28": index_bytes_28, "rows_28": rows_28,
            "original_blake3_after": original_blake3_after,
            "window_samples": windows.len(), "painted_rows": rows,
            "window_p50_ms": windows[50], "window_p95_ms": windows[95],
            "retained_status": retained, "final_status": status(),
        })
    );
}

fn window_samples(pages: &Pages) -> (Vec<f64>, usize) {
    let mut samples = Vec::new();
    let mut rows = 0;
    for round in 0..101 {
        let start = round * pages.rows().saturating_sub(50) / 100;
        let clock = Instant::now();
        rows += pages.window(start);
        samples.push(clock.elapsed().as_secs_f64() * 1000.0);
    }
    (samples, rows)
}

fn source(case: &str, mib: usize) -> String {
    if let Some(path) = case.strip_prefix("file:") {
        return fs::read_to_string(path).expect("captured original");
    }
    let (case, prefix) = case
        .strip_prefix("plain:")
        .map_or((case, "\x1b[31m"), |case| (case, ""));
    let atom = match case {
        "literal" => "x",
        "words" => "ordinary words, [punctuation]: x-y; more text  ",
        "unicode" => "界🙂é\t",
        "ligatures" => "لاא‍לⵏ⵿ⴾᨕᨗ‍ᨐꓹꓼ🇫🇷👩‍💻",
        "mixed" => "words, 界🙂é\t\x1b[32mgreen: [a-b]\x1b[31m ",
        _ => panic!("unknown explicit probe case: {case}"),
    };
    let available = mib * 1024 * 1024 - prefix.len();
    let mut source = prefix.to_string();
    source.push_str(&atom.repeat(available / atom.len()));
    source.push_str(&"x".repeat(available % atom.len()));
    source
}

fn status() -> String {
    fs::read_to_string("/proc/self/status")
        .expect("process status")
        .lines()
        .filter(|line| line.starts_with("VmRSS:") || line.starts_with("VmHWM:"))
        .collect::<Vec<_>>()
        .join("; ")
}
