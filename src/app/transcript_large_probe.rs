//! Measure one prototype pinned original and the existing full-block formatter on real captures.

#[path = "transcript_ansi_probe.rs"]
mod ansi_probe;

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::mem::size_of;
use std::path::Path;
use std::time::Instant;

use carlos_client::Bodies;
use carlos_proto::bytes::ByteLength;
use carlos_proto::ids::SessionId;
use carlos_proto::session::Entry;
use carlos_proto::source::{Source, SourceChunk};
use serde_json::{json, Value};

use super::socket_probe::fingerprint;
use super::BlockBody;
use crate::app::transcript_input::RenderInput;
use crate::app::transcript_styles::RenderCountCache;
use crate::app::view_models::{DisplayKind, RenderedLine, StyledSegment};
use crate::clipboard::{clipboard_backend_label, try_copy_clipboard, ClipboardBackend};

struct Capture {
    entry: Entry,
    source: Source,
    text: String,
    before: Value,
    pinned: Value,
}

struct Sample {
    rows: usize,
    count_ms: f64,
    materialize_ms: f64,
    total_ms: f64,
    owned_capacity_bytes: usize,
    counted: Value,
    painted: Value,
}

#[test]
#[ignore = "capture with scripts/probe-large-source.py; run each case/width in its own release process"]
fn large_source_layout_probe() {
    let directory = std::env::var("CARLOS_LARGE_PROBE").expect("capture directory");
    let name = std::env::var("CARLOS_LARGE_CASE").expect("ansi or markdown case");
    let width = setting("CARLOS_LARGE_WIDTH", 120);
    let samples = setting("CARLOS_LARGE_SAMPLES", 16).clamp(1, 32);
    let root = Path::new(&directory);
    let manifest: Value = load_json(&root.join("manifest.json"));
    let case = manifest["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["name"] == name)
        .expect("captured case");
    let mut capture = hydrate(root, case);
    let mut raster = Vec::new();
    let baseline = render(&capture, width, &mut raster);
    let digest = fingerprint(&raster);
    let tail = if name == "ansi" {
        "large ansi tail stays red"
    } else {
        "Large Markdown tail link"
    };
    let last_rows: Vec<_> = raster.iter().rev().take(10).collect();
    let visible_tail: String = last_rows
        .iter()
        .rev()
        .flat_map(|line| line.text.chars())
        .filter(|character| !character.is_whitespace())
        .collect();
    let expected_tail: String = tail
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert!(visible_tail.contains(&expected_tail));
    let copy = copy_original(&capture);
    let initial_capacity = capture.text.capacity();
    let append = append_samples(&mut capture, width, samples, &mut raster);
    let resize = resize_samples(&capture, samples, &mut raster);
    let metrics = json!({"run_id":std::env::var("CARLOS_LARGE_RUN_ID").ok(),"case":name,"width":width,"source_bytes":case["bytes"],
        "source_initial_capacity_bytes":initial_capacity,"source_final_capacity_bytes":capture.text.capacity(),
        "before":capture.before,"pinned":capture.pinned,"baseline":baseline.json(),
        "render_digest":digest,"clipboard":copy,"append":summarize(&append),"resize":summarize(&resize),
        "final":memory(),"scope":"offline formatter with real validated source; not socket latency or production storage"});
    let destination = root.join(&name).join(format!("width-{width}.json"));
    std::fs::write(
        destination,
        serde_json::to_vec_pretty(&metrics).expect("metrics"),
    )
    .expect("save metrics");
    println!("LARGE_SOURCE_PROBE {metrics}");
}

fn hydrate(root: &Path, case: &Value) -> Capture {
    let before = memory();
    let path = root.join(case["name"].as_str().expect("case name"));
    let entry: Entry = serde_json::from_value(load_json(&path.join("entry.json"))).expect("entry");
    let source: Source = serde_json::from_value(case["source"].clone()).expect("source");
    let session: SessionId = serde_json::from_value(case["session"].clone()).expect("session");
    let mut windows = Bodies::default();
    let token = windows
        .open_source(session, entry.id.clone(), source.revision)
        .expect("suffix validator");
    let mut text =
        String::with_capacity(usize::try_from(source.bytes.get()).expect("host body size"));
    for line in BufReader::new(File::open(path.join("chunks.jsonl")).expect("wire chunks")).lines()
    {
        let chunk: SourceChunk = serde_json::from_str(&line.expect("wire line")).expect("chunk");
        assert_eq!(chunk.source, source);
        // Keep validation in the existing pure cache; this prototype owns the full string separately.
        assert!(windows
            .apply_source(&token, chunk.clone())
            .expect("checked exact cursor/revision/offset"));
        text.push_str(&chunk.text);
    }
    assert_eq!(text.len() as u64, source.bytes.get());
    let window = windows.source(&token).expect("validated source end");
    assert_eq!(window.end().get(), source.bytes.get());
    assert!(!window.has_more());
    drop(windows);
    Capture {
        entry,
        source,
        text,
        before,
        pinned: memory(),
    }
}

fn render(capture: &Capture, width: usize, previous: &mut Vec<RenderedLine>) -> Sample {
    let started = Instant::now();
    // Include retiring the prior full raster in the append/resize measurement.
    drop(std::mem::take(previous));
    let input = RenderInput::from_client(&capture.entry, Some((&capture.source, &capture.text)))
        .expect("complete validated original");
    let body = BlockBody::for_input(input, DisplayKind::Full, width);
    let count_started = Instant::now();
    let rows = body.count_input(&mut RenderCountCache::new(), input);
    let count_ms = milliseconds(count_started);
    let counted = memory();
    let paint_started = Instant::now();
    body.append_input(previous, input);
    let materialize_ms = milliseconds(paint_started);
    let total_ms = milliseconds(started);
    assert_eq!(previous.len(), rows);
    Sample {
        rows,
        count_ms,
        materialize_ms,
        total_ms,
        owned_capacity_bytes: raster_capacity(previous, previous.capacity()),
        counted,
        painted: memory(),
    }
}

fn append_samples(
    capture: &mut Capture,
    width: usize,
    count: usize,
    raster: &mut Vec<RenderedLine>,
) -> Vec<Sample> {
    (0..count)
        .map(|index| {
            let update_started = Instant::now();
            capture
                .text
                .push_str(&format!("\nappend measurement {index}\n"));
            capture.source.bytes = ByteLength::new(capture.text.len() as u64);
            if let Some(source) = &mut capture.entry.source {
                *source = capture.source.clone();
            }
            let update_ms = milliseconds(update_started);
            let mut sample = render(capture, width, raster);
            sample.total_ms += update_ms;
            sample
        })
        .collect()
}

fn resize_samples(capture: &Capture, count: usize, raster: &mut Vec<RenderedLine>) -> Vec<Sample> {
    (0..count)
        .map(|index| render(capture, if index % 2 == 0 { 28 } else { 120 }, raster))
        .collect()
}

fn copy_original(capture: &Capture) -> Value {
    if std::env::var("CARLOS_LARGE_COPY").as_deref() != Ok("1") {
        return json!({"skipped":"set CARLOS_LARGE_COPY=1 only with a private clipboard shim"});
    }
    let path = std::env::var("CARLOS_LARGE_COPY_PATH").expect("private clipboard output path");
    let started = Instant::now();
    let backend = try_copy_clipboard(&capture.text);
    assert!(
        matches!(backend, ClipboardBackend::Program(_)),
        "private clipboard program must succeed"
    );
    json!({"path":path,"bytes":capture.text.len(),"ms":milliseconds(started),"backend":clipboard_backend_label(backend)})
}

fn raster_capacity(lines: &[RenderedLine], capacity: usize) -> usize {
    capacity * size_of::<RenderedLine>()
        + lines
            .iter()
            .map(|line| {
                line.text.capacity()
                    + line.styled_segments.capacity() * size_of::<StyledSegment>()
                    + line
                        .styled_segments
                        .iter()
                        .map(|segment| segment.text.capacity())
                        .sum::<usize>()
            })
            .sum::<usize>()
}

impl Sample {
    fn json(&self) -> Value {
        json!({"rows":self.rows,"count_ms":self.count_ms,"materialize_ms":self.materialize_ms,
            "total_ms":self.total_ms,"raster_vec_string_capacities_bytes":self.owned_capacity_bytes,
            "after_count":self.counted,"after_raster":self.painted})
    }
}

fn summarize(samples: &[Sample]) -> Value {
    let mut times: Vec<_> = samples.iter().map(|sample| sample.total_ms).collect();
    times.sort_by(f64::total_cmp);
    let p95 = ((times.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
    json!({"p95_ms":times[p95],"samples":samples.iter().map(Sample::json).collect::<Vec<_>>()})
}

fn memory() -> Value {
    let status = std::fs::read_to_string("/proc/self/status").expect("Linux memory probe");
    let mut values = serde_json::Map::new();
    for line in status.lines() {
        if let Some((key, value)) = line.split_once(':') {
            if matches!(key, "VmRSS" | "VmHWM" | "RssAnon" | "RssFile") {
                values.insert(
                    key.into(),
                    json!(value
                        .split_whitespace()
                        .next()
                        .expect("memory value")
                        .parse::<u64>()
                        .expect("KiB")),
                );
            }
        }
    }
    Value::Object(values)
}

fn milliseconds(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.
}

fn setting(name: &str, default: usize) -> usize {
    std::env::var(name).map_or(default, |value| {
        value.parse().expect("numeric probe setting")
    })
}

fn load_json(path: &Path) -> Value {
    serde_json::from_reader(File::open(path).expect("capture file")).expect("capture JSON")
}
