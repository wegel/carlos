//! Measure the actual formatter worker after bounded Client source admission.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use carlos_client::{Bodies, BodyToken};
use serde_json::{json, Value};

use super::super::{Formatter, Reply, Request};
use super::{barrier, cache, feed, push};
use crate::app::layout_state::TranscriptTheme;
use crate::app::render_entry_cache::RowFormat;
use crate::app::transcript_input::RenderInput;
use crate::app::view_models::{RenderedLine, StyledSegment};
use carlos_core::session::models::{MessageKind, Role};

const SAMPLES: usize = 16;

#[derive(Default)]
struct Timings {
    append: Vec<f64>,
    wider: Vec<f64>,
    narrower: Vec<f64>,
}

#[derive(Default)]
struct HighWater {
    rows: usize,
    payload: usize,
}

impl HighWater {
    fn observe(&mut self, prepared: crate::app::render_entry_cache::PreparedRow) -> usize {
        let lines = prepared.lines.expect("painted worker frame");
        let rows = lines.retained().collect::<Vec<_>>();
        let payload = rows
            .iter()
            .map(|line| {
                size_of::<RenderedLine>()
                    + line.text.capacity()
                    + line.styled_segments.capacity() * size_of::<StyledSegment>()
                    + line
                        .styled_segments
                        .iter()
                        .map(|part| part.text.capacity())
                        .sum::<usize>()
            })
            .sum::<usize>();
        self.rows = self.rows.max(rows.len());
        self.payload = self.payload.max(payload);
        assert!(
            rows.len() <= 64,
            "hidden rows must never become a full raster"
        );
        prepared.count
    }
}

fn show(formatter: &mut Formatter, markdown: bool, width: usize, first: usize) -> u64 {
    formatter
        .show(
            RenderInput {
                role: if markdown {
                    Role::Assistant
                } else {
                    Role::ToolOutput
                },
                kind: MessageKind::Plain,
                text: "",
                file_path: None,
                label: None,
                label_generated: false,
                failed: false,
            },
            RowFormat {
                width,
                compact: false,
                theme: TranscriptTheme::Gutter,
                hidden: false,
            },
            None,
            first,
            48,
        )
        .expect("actual viewport request")
}

fn frame(formatter: &mut Formatter, view: u64, bytes: usize, high: &mut HighWater) -> usize {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        for reply in formatter.poll() {
            match reply {
                Reply::Frame {
                    view: found,
                    source,
                    prepared,
                    ..
                } if found == view && source.bytes.get() == bytes as u64 => {
                    return high.observe(prepared);
                }
                Reply::Failed { error, .. } => panic!("worker failed: {error:?}"),
                _ => {}
            }
        }
        assert!(Instant::now() < deadline, "matching worker frame");
        thread::yield_now();
    }
}

fn tail_frame(
    formatter: &mut Formatter,
    markdown: bool,
    width: usize,
    bytes: usize,
    high: &mut HighWater,
) -> usize {
    let view = show(formatter, markdown, width, 0);
    let count = frame(formatter, view, bytes, high);
    let view = show(formatter, markdown, width, count.saturating_sub(48));
    assert_eq!(frame(formatter, view, bytes, high), count);
    count
}

fn load(
    formatter: &mut Formatter,
    bodies: &mut Bodies,
    token: &BodyToken,
    path: &Path,
) -> (usize, blake3::Hasher, usize) {
    let mut file = File::open(path).expect("captured source");
    let total = file.metadata().expect("source length").len() as usize;
    assert!(total > 0, "provide a nonempty captured source");
    let mut buffer = vec![0_u8; 65536];
    let (mut carry, mut offset, mut peak) = (0, 0, 0);
    let mut hash = blake3::Hasher::new();
    loop {
        let count = file.read(&mut buffer[carry..]).expect("source bytes");
        if count == 0 && carry == 0 {
            break;
        }
        let used = carry + count;
        let length = match std::str::from_utf8(&buffer[..used]) {
            Ok(text) => text.len(),
            Err(error) if error.error_len().is_none() && count > 0 => error.valid_up_to(),
            Err(error) => panic!("invalid captured UTF-8: {error}"),
        };
        assert!(
            length > 0,
            "a complete scalar must fit inside one source piece"
        );
        let text = std::str::from_utf8(&buffer[..length]).expect("checked UTF-8");
        push(bodies, token, offset, text, total);
        feed(formatter, bodies, offset + length);
        hash.update(text.as_bytes());
        offset += length;
        let window = bodies.source(token).expect("accepted suffix");
        peak = peak.max((window.end().get() - window.start().get()) as usize);
        buffer.copy_within(length..used, 0);
        carry = used - length;
    }
    assert_eq!(offset, total);
    barrier(formatter);
    (offset, hash, peak)
}

fn summarize(values: &[f64]) -> Value {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let index = (sorted.len() * 95).div_ceil(100) - 1;
    json!({"samples_ms":values,"p95_ms":sorted[index]})
}

fn memory() -> Value {
    let status = std::fs::read_to_string("/proc/self/status").expect("Linux process memory");
    let get = |key: &str| {
        status
            .lines()
            .find_map(|line| {
                let value = line.strip_prefix(key)?.split_whitespace().next()?;
                value.parse::<u64>().ok()
            })
            .expect("kernel memory counter")
    };
    json!({"rss_kib":get("VmRSS:"),"peak_rss_kib":get("VmHWM:")})
}

fn samples(
    formatter: &mut Formatter,
    bodies: &mut Bodies,
    token: &BodyToken,
    markdown: bool,
    bytes: &mut usize,
    hash: &mut blake3::Hasher,
    high: &mut HighWater,
) -> Timings {
    let mut times = Timings::default();
    for index in 0..SAMPLES {
        let addition = format!("\nappend measurement {index}\n");
        let started = Instant::now();
        push(bodies, token, *bytes, &addition, *bytes + addition.len());
        feed(formatter, bodies, *bytes + addition.len());
        *bytes += addition.len();
        hash.update(addition.as_bytes());
        tail_frame(formatter, markdown, 28, *bytes, high);
        times.append.push(started.elapsed().as_secs_f64() * 1000.0);
        let started = Instant::now();
        tail_frame(formatter, markdown, 120, *bytes, high);
        times.wider.push(started.elapsed().as_secs_f64() * 1000.0);
        let started = Instant::now();
        tail_frame(formatter, markdown, 28, *bytes, high);
        times
            .narrower
            .push(started.elapsed().as_secs_f64() * 1000.0);
    }
    times
}

fn digest(formatter: &mut Formatter) -> blake3::Hash {
    let job = formatter.producer.as_ref().expect("current job").job.id;
    formatter
        .sender
        .send(Request::Digest(job))
        .expect("ordered source digest");
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        for reply in formatter.poll() {
            if let Reply::Digest { job: found, digest } = reply {
                assert_eq!(found, job);
                return digest;
            }
        }
        assert!(Instant::now() < deadline, "worker digest");
        thread::yield_now();
    }
}

#[test]
#[ignore = "requires a captured large source and a quiet host"]
fn formatter_worker_source_metrics() {
    let Ok(source) = std::env::var("CARLOS_FORMATTER_SOURCE_PATH") else {
        return;
    };
    let result = std::env::var("CARLOS_FORMATTER_RESULTS").expect("fresh result path");
    let run_id = std::env::var("CARLOS_FORMATTER_RUN_ID").expect("fresh run identity");
    let case = std::env::var("CARLOS_FORMATTER_CASE").expect("ansi or markdown");
    assert!(matches!(case.as_str(), "ansi" | "markdown"));
    let markdown = case == "markdown";
    let (mut bodies, token) = cache();
    let mut formatter = Formatter::new().expect("real formatter worker");
    formatter
        .open(token.clone(), carlos_proto::output::OutputRevision::new(3))
        .expect("source job");
    let process_before_source = memory();
    let started = Instant::now();
    let (mut bytes, mut hash, cache_peak) =
        load(&mut formatter, &mut bodies, &token, Path::new(&source));
    let admission_ms = started.elapsed().as_secs_f64() * 1000.0;
    let initial_bytes = bytes;
    let before_layout = memory();
    let started = Instant::now();
    let mut high = HighWater::default();
    let rows = tail_frame(&mut formatter, markdown, 28, bytes, &mut high);
    let initial_layout_ms = started.elapsed().as_secs_f64() * 1000.0;
    let indexed_memory = memory();
    let times = samples(
        &mut formatter,
        &mut bodies,
        &token,
        markdown,
        &mut bytes,
        &mut hash,
        &mut high,
    );
    assert_eq!(
        digest(&mut formatter),
        hash.finalize(),
        "all admitted source bytes survive"
    );
    let metrics = json!({"run_id":run_id,"case":if markdown {"markdown"}else{"ansi"},
        "source_path":source,"initial_source_bytes":initial_bytes,"final_source_bytes":bytes,
        "source_blake3":hash.finalize().to_hex().as_str(),"admission_ms":admission_ms,
        "initial_layout_ms":initial_layout_ms,"logical_rows":rows,"cache_data_bytes_high_water":cache_peak,
        "painted_rows_high_water":high.rows,"painted_row_payload_capacity_bytes_high_water":high.payload,
        "payload_excludes":"spare row-vector slots and allocator overhead; kernel RSS includes both",
        "process_before_source":process_before_source,"source_only_memory":before_layout,"indexed_memory":indexed_memory,"final_memory":memory(),
        "append":summarize(&times.append),"resize_28_to_120":summarize(&times.wider),
        "resize_120_to_28":summarize(&times.narrower),
        "scope":"actual formatter worker and checked Client cache; admission finishes before first layout; samples await count/head and final tail frames; socket/terminal paint overhead measured separately"});
    std::fs::write(
        result,
        serde_json::to_vec_pretty(&metrics).expect("metrics JSON"),
    )
    .expect("fresh metrics");
}
