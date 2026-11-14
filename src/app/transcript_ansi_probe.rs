//! Compare a private checkpoint candidate with the captured full-raster baseline.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use ratatui::style::Color;
use serde_json::{json, Value};

use super::{copy_original, hydrate, load_json, memory, milliseconds, raster_capacity, setting};
use crate::app::transcript_body::ansi_pages::AnsiPages;
use crate::app::transcript_input::RenderInput;

#[test]
#[ignore = "run with a real validated ANSI capture, isolated release process and private clipboard"]
fn large_ansi_checkpoint_probe() {
    let directory = std::env::var("CARLOS_LARGE_PROBE").expect("capture directory");
    let root = Path::new(&directory);
    let width = setting("CARLOS_LARGE_WIDTH", 120);
    let count = setting("CARLOS_LARGE_SAMPLES", 16).clamp(1, 32);
    let manifest = load_json(&root.join("manifest.json"));
    let case = manifest["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["name"] == "ansi")
        .expect("ANSI capture");
    let mut capture = hydrate(root, case);
    let copy = copy_original(&capture);
    let role = RenderInput::from_client(&capture.entry, Some((&capture.source, &capture.text)))
        .expect("complete validated original")
        .role;
    let started = Instant::now();
    let text = Arc::new(std::mem::take(&mut capture.text));
    let mut pages = AnsiPages::new(text, role, width).expect("checkpoint index");
    let index_ms = milliseconds(started);
    check_rows(root, width, case, &pages);
    let initial_rows = pages.rows();
    let initial_index_bytes = pages.index_capacity_bytes();
    let indexed_memory = memory();
    let viewport = paint(&pages);
    let append: Vec<_> = (0..count)
        .map(|index| {
            let started = Instant::now();
            let addition = if case["single_ascii_token"] == true {
                "x".to_string()
            } else {
                format!("\nappend measurement {index}\n")
            };
            pages.append(&addition).expect("append accepted original");
            let mut sample = paint(&pages);
            sample["total_ms"] = json!(milliseconds(started));
            sample
        })
        .collect();
    let mut resize = Vec::new();
    for index in 0..count {
        let started = Instant::now();
        pages = pages
            .resize(if index % 2 == 0 { 28 } else { 120 })
            .expect("resize original");
        let mut sample = paint(&pages);
        sample["total_ms"] = json!(milliseconds(started));
        resize.push(sample);
    }
    let metrics = json!({"run_id":std::env::var("CARLOS_LARGE_RUN_ID").ok(),"width":width,"source_bytes":case["bytes"],"rows":initial_rows,
        "index_ms":index_ms,"index_capacity_bytes":initial_index_bytes,"source_memory":capture.pinned,
        "indexed_memory":indexed_memory,"viewport":viewport,"clipboard":copy,
        "append":summary(append),"resize":summary(resize),"final_memory":memory(),
        "scope":"private ANSI checkpoint prototype; not production or socket latency"});
    save_metrics(root, width, &metrics);
}

fn check_rows(root: &Path, width: usize, case: &Value, pages: &AnsiPages) {
    if case["single_ascii_token"] == true {
        let bytes = case["bytes"].as_u64().expect("bytes") as usize;
        assert_eq!(pages.rows(), (bytes - 5).div_ceil(width));
        let tail = pages
            .window(pages.rows().saturating_sub(50), 50)
            .expect("literal tail");
        assert!(tail
            .iter()
            .map(|line| line.text.as_str())
            .collect::<String>()
            .ends_with("LONGTAIL"));
        assert!(tail
            .iter()
            .flat_map(|line| &line.styled_segments)
            .all(|span| span.style.fg == Some(Color::Red)));
    } else {
        let baseline = load_json(&root.join("ansi").join(format!("width-{width}.json")));
        assert_eq!(
            pages.rows() as u64,
            baseline["baseline"]["rows"]
                .as_u64()
                .expect("baseline rows")
        );
    }
}

fn save_metrics(root: &Path, width: usize, metrics: &Value) {
    std::fs::write(
        root.join("ansi").join(format!("checkpoint-{width}.json")),
        serde_json::to_vec_pretty(metrics).expect("metrics"),
    )
    .expect("save metrics");
    println!("ANSI_CHECKPOINT_PROBE {metrics}");
}

fn paint(pages: &AnsiPages) -> Value {
    let started = Instant::now();
    let lines = pages
        .window(pages.rows().saturating_sub(50), 50)
        .expect("visible tail");
    json!({"total_ms":milliseconds(started),"visible_rows":lines.len(),
        "rows":pages.rows(),"raster_capacity_bytes":raster_capacity(&lines,lines.capacity()),
        "index_capacity_bytes":pages.index_capacity_bytes(),"memory":memory()})
}

fn summary(samples: Vec<Value>) -> Value {
    let mut times: Vec<_> = samples
        .iter()
        .map(|sample| sample["total_ms"].as_f64().expect("ms"))
        .collect();
    times.sort_by(f64::total_cmp);
    let index = ((times.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
    json!({"p95_ms":times[index],"samples":samples})
}
