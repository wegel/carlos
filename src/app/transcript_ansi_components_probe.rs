//! Time full-line parser, source-map, wrap and style-cursor work separately.

use std::env;
use std::fs;
use std::mem::size_of;
use std::time::Instant;

use ratatui::style::Style;
use serde_json::json;
use textwrap::core::Word;
use textwrap::word_splitters::split_words;
use textwrap::{Options, WordSplitter};

use super::super::seed::parse_mapped;
use super::super::source_ranges::SourceRanges;
use super::super::text_window::{
    compact_word_bytes, visit_legacy_rows, visit_rows, wide_word_bytes,
};
use super::super::SegmentCursor;
use super::source;
use crate::app::text::visual_width;
use crate::app::transcript_styles::styled_plain_text;

#[test]
#[ignore = "separate bounded release component diagnostic"]
fn measure_long_line_components() {
    let (case, mib, source, appended) = source_case();
    let reference = env::var_os("CARLOS_ANSI_INDEX_REFERENCE").is_some();
    let mut memory = vec![snapshot("source")];
    let started = Instant::now();
    let batch = parse_mapped(&source, Style::default()).expect("parser");
    let parse_ms = started.elapsed().as_secs_f64() * 1000.0;
    memory.push(snapshot("parsed"));
    let [line] = batch.lines.as_slice() else {
        panic!("one complete physical line");
    };
    let started = Instant::now();
    let plain = styled_plain_text(line);
    let plain_ms = started.elapsed().as_secs_f64() * 1000.0;
    memory.push(snapshot("plain"));
    let started = Instant::now();
    let mut ranges = SourceRanges::default();
    let expanded = ranges.append_line(&batch.ranges[0], &source, &plain);
    let map_ms = started.elapsed().as_secs_f64() * 1000.0;
    memory.push(snapshot("mapped"));
    let started = Instant::now();
    let mut cells = Vec::new();
    if reference {
        visit_legacy_rows(&expanded, 28, |_, text| cells.push(visual_width(text)));
    } else {
        visit_rows(&expanded, 28, |_, text| cells.push(visual_width(text)));
    }
    let wrap_ms = started.elapsed().as_secs_f64() * 1000.0;
    memory.push(snapshot("wrapped"));
    let started = Instant::now();
    let mut cursor = SegmentCursor::new(line);
    for cells in &cells {
        cursor.take(*cells, false);
    }
    let cursor_ms = started.elapsed().as_secs_f64() * 1000.0;
    memory.push(snapshot("cursor"));
    let options = Options::new(28)
        .break_words(false)
        .word_splitter(WordSplitter::NoHyphenation);
    let words = split_words(
        options.word_separator.find_words(&expanded),
        &options.word_splitter,
    )
    .count();
    println!(
        "ANSI_COMPONENTS {}",
        json!({
            "case":case,"mib":mib,"appended":appended,"source_bytes":source.len(),"reference":reference,
            "rows":cells.len(),"parse_ms":parse_ms,"plain_ms":plain_ms,"map_ms":map_ms,
            "wrap_ms":wrap_ms,"cursor_ms":cursor_ms,"map_capacity_bytes":ranges.capacity_bytes(),
            "width_scan_bytes":cursor.width_scan_bytes(),"style_spans":line.len(),
            "word_count":words,"upstream_word_bytes":size_of::<Word<'_>>(),
            "compact_word_bytes":compact_word_bytes(),"wide_word_bytes":wide_word_bytes(),"memory":memory,
        })
    );
}

fn source_case() -> (String, String, String, bool) {
    let case = env::var("CARLOS_ANSI_INDEX_CASE").expect("explicit component case");
    let mib = env::var("CARLOS_ANSI_INDEX_MIB").expect("explicit component size");
    let mut source = source(&case, mib.parse().expect("MiB"));
    let appended = env::var_os("CARLOS_ANSI_INDEX_APPENDED").is_some();
    if appended {
        source.push_str(" next, 界\t🙂");
    }
    (case, mib, source, appended)
}

fn snapshot(phase: &str) -> serde_json::Value {
    let status = fs::read_to_string("/proc/self/status").expect("phase memory");
    let mut data = serde_json::Map::new();
    data.insert("phase".into(), json!(phase));
    for line in status
        .lines()
        .filter(|line| line.starts_with("VmRSS:") || line.starts_with("VmHWM:"))
    {
        let mut fields = line.split_whitespace();
        let key = fields.next().expect("memory key").trim_end_matches(':');
        let value: usize = fields.next().expect("KiB").parse().expect("memory value");
        data.insert(key.into(), json!(value));
    }
    serde_json::Value::Object(data)
}
