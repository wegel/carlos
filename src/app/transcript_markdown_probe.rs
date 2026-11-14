//! Compare whole Markdown parse/index storage with the complete-raster baseline on real captures.

use std::path::Path;
use std::time::Instant;

use carlos_core::session::models::Role;
use serde_json::{json, Value};

use super::{
    copy_original, hydrate, load_json, memory, milliseconds, raster_capacity, setting,
    summarize_values,
};
use crate::app::transcript_body::document::DocumentFrame;
use crate::app::transcript_input::RenderInput;

#[test]
#[ignore = "real captured Markdown, private clipboard and isolated release process required"]
fn large_markdown_document_probe() {
    let directory = std::env::var("CARLOS_LARGE_PROBE").expect("capture directory");
    let name = std::env::var("CARLOS_LARGE_CASE").expect("Markdown capture name");
    let root = Path::new(&directory);
    let width = setting("CARLOS_LARGE_WIDTH", 120);
    let count = setting("CARLOS_LARGE_SAMPLES", 16).clamp(1, 32);
    let manifest = load_json(&root.join("manifest.json"));
    let case = manifest["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["name"] == name)
        .expect("captured case");
    let mut capture = hydrate(root, case);
    let input = RenderInput::from_client(&capture.entry, Some((&capture.source, &capture.text)))
        .expect("complete original");
    assert_eq!(input.role, Role::Assistant);
    let copy = copy_original(&capture);
    let (initial, baseline) = build(&capture.text, width, None);
    let expected = load_json(&root.join(&name).join(format!("width-{width}.json")));
    assert_eq!(
        initial.rows() as u64,
        expected["baseline"]["rows"]
            .as_u64()
            .expect("baseline row count")
    );
    let mut document = Some(initial);
    let append = (0..count)
        .map(|index| {
            let started = Instant::now();
            capture
                .text
                .push_str(&format!("\nappend measurement {index}\n"));
            let (next, mut sample) = build(&capture.text, width, document.take());
            document = Some(next);
            sample["total_ms"] = json!(milliseconds(started));
            sample
        })
        .collect();
    let resize = (0..count)
        .map(|index| {
            let (next, sample) = build(
                &capture.text,
                if index % 2 == 0 { 28 } else { 120 },
                document.take(),
            );
            document = Some(next);
            sample
        })
        .collect();
    let metrics = json!({"run_id":std::env::var("CARLOS_LARGE_RUN_ID").ok(),"case":name,"width":width,
        "source_bytes":case["bytes"],"source_memory":capture.pinned,"baseline":baseline,"clipboard":copy,
        "append":summarize_values(append),"resize":summarize_values(resize),"final_memory":memory(),
        "scope":"test-only whole parsed Markdown document; not production or socket latency"});
    save(root, &name, width, &metrics);
}

fn build(text: &str, width: usize, previous: Option<DocumentFrame>) -> (DocumentFrame, Value) {
    let started = Instant::now();
    drop(previous);
    let parse_started = Instant::now();
    let mut document = DocumentFrame::parse(text, width);
    let parse_ms = milliseconds(parse_started);
    let parsed_memory = memory();
    let index_started = Instant::now();
    document.index();
    let index_ms = milliseconds(index_started);
    let indexed_memory = memory();
    let paint_started = Instant::now();
    let lines = document.window(document.rows().saturating_sub(50), 50);
    let paint_ms = milliseconds(paint_started);
    let total_ms = milliseconds(started);
    let sample = json!({"rows":document.rows(),"parse_ms":parse_ms,"index_ms":index_ms,"paint_ms":paint_ms,
        "total_ms":total_ms,"document_capacity_bytes":document.document_capacity_bytes(),
        "index_capacity_bytes":document.index_capacity_bytes(),"viewport_capacity_bytes":raster_capacity(&lines,lines.capacity()),
        "parsed_memory":parsed_memory,"indexed_memory":indexed_memory,"painted_memory":memory()});
    (document, sample)
}

fn save(root: &Path, name: &str, width: usize, metrics: &Value) {
    std::fs::write(
        root.join(name).join(format!("document-{width}.json")),
        serde_json::to_vec_pretty(metrics).expect("metrics"),
    )
    .expect("save metrics");
    println!("MARKDOWN_DOCUMENT_PROBE {metrics}");
}
