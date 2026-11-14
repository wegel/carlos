//! Replay captured real-socket source chunks through client windows and borrowed body rendering.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Instant;

use carlos_client::{Bodies, BodyToken};
use carlos_core::session::entry_id::EntryId;
use carlos_core::session::models::Message;
use carlos_proto::ids::SessionId;
use carlos_proto::session::Entry;
use carlos_proto::source::{Source, SourceChunk};
use serde_json::{json, Value};

use super::BlockBody;
use crate::app::transcript_input::RenderInput;
use crate::app::transcript_styles::RenderCountCache;
use crate::app::view_models::{DisplayKind, DisplayMessage, RenderedLine};

#[test]
#[ignore = "capture real socket cases with scripts/probe-client-render.py first"]
fn socket_render_probe() {
    let directory = std::env::var("CARLOS_RENDER_PROBE").expect("probe artifact directory");
    let root = Path::new(&directory);
    let manifest: Value =
        serde_json::from_reader(File::open(root.join("manifest.json")).expect("manifest"))
            .expect("JSON manifest");
    for case in manifest["cases"].as_array().expect("captured cases") {
        run_case(root, case);
    }
}

fn run_case(root: &Path, case: &Value) {
    let path = root.join(case["name"].as_str().expect("case name"));
    let entry: Entry = serde_json::from_reader(File::open(path.join("entry.json")).expect("entry"))
        .expect("typed row");
    let session: SessionId =
        serde_json::from_value(case["session"].clone()).expect("session address");
    let descriptor: Source =
        serde_json::from_value(case["source"].clone()).expect("source metadata");
    let source = &descriptor;
    let mut bodies = Bodies::default();
    let token = bodies
        .open_source(session.clone(), entry.id.clone(), source.revision)
        .expect("bounded source window");
    let mut documents = Bodies::default();
    let document = documents
        .open_source_document(session, entry.id.clone(), source.revision)
        .expect("contiguous source lifetime");
    let before = rss();
    let started = Instant::now();
    assert!(
        source.bytes.get() <= 8 * 1024 * 1024,
        "prototype cap, not a product limit"
    );
    for line in BufReader::new(File::open(path.join("chunks.jsonl")).expect("wire chunks")).lines()
    {
        let chunk: SourceChunk =
            serde_json::from_str(&line.expect("chunk line")).expect("typed original source");
        assert_eq!(&chunk.source, source);
        documents
            .apply_source(&document, chunk.clone())
            .expect("contiguous source chunk");
        bodies
            .apply_source(&token, chunk)
            .expect("checked client cursor");
    }
    let hydrate_ms = started.elapsed().as_secs_f64() * 1000.;
    reject_suffix(&bodies, &token, &entry, source);
    drop(bodies);
    let text = documents
        .source(&document)
        .expect("complete source")
        .document()
        .expect("one borrowed complete document");
    assert_eq!(text.len() as u64, source.bytes.get());
    let after = rss();
    let input = RenderInput::from_client(&entry, Some((source, text)))
        .expect("contiguous complete candidate");
    assert_eq!(input.text.as_ptr(), text.as_ptr());
    let message = Message {
        id: EntryId::from_wire(&entry.id),
        role: input.role,
        kind: input.kind,
        file_path: input.file_path.map(str::to_owned),
        tool_output: None,
        text: text.to_owned(),
    };
    for width in [28, 120] {
        compare(input, &message, width, case, hydrate_ms, before, after);
    }
}

fn reject_suffix(bodies: &Bodies, token: &BodyToken, entry: &Entry, source: &Source) {
    let window = bodies.source(token).expect("source window");
    assert!(
        window.start().get() > 0,
        "fixtures exceed the current client window"
    );
    let suffix: String = window.pieces().collect();
    assert!(RenderInput::from_client(entry, Some((source, &suffix))).is_none());
    drop(suffix);
}

fn compare(
    input: RenderInput<'_>,
    message: &Message,
    width: usize,
    case: &Value,
    hydrate_ms: f64,
    before: Option<u64>,
    after: Option<u64>,
) {
    let started = Instant::now();
    let baseline = BlockBody::new(DisplayMessage::full(message), width);
    let count = baseline.count(&mut RenderCountCache::new(), message);
    let mut lines = Vec::new();
    baseline.append(&mut lines, message);
    assert_eq!(lines.len(), count);
    let expected = fingerprint(&lines);
    drop(lines);
    let baseline_ms = started.elapsed().as_secs_f64() * 1000.;
    let started = Instant::now();
    let candidate = BlockBody::for_input(input, DisplayKind::Full, width);
    assert_eq!(
        candidate.count_input(&mut RenderCountCache::new(), input),
        count
    );
    let mut lines = Vec::new();
    candidate.append_input(&mut lines, input);
    assert_eq!(
        fingerprint(&lines),
        expected,
        "text/styles/wraps must match"
    );
    println!(
        "RENDER_PROBE {}",
        json!({"case":case["name"],"bytes":message.text.len(),"width":width,
        "lines":count,"hydrate_ms":hydrate_ms,"baseline_count_draw_ms":baseline_ms,
        "borrowed_count_draw_ms":started.elapsed().as_secs_f64()*1000.,"rss_before_kib":before,"rss_source_kib":after,
        "rss_rendered_kib":rss(),"render_digest":expected})
    );
}

pub(super) fn fingerprint(lines: &[RenderedLine]) -> String {
    let mut hash = blake3::Hasher::new();
    for line in lines {
        hash.update(
            &serde_json::to_vec(
                &json!({"text":line.text,"cells":line.cells,"role":format!("{:?}",line.role),
            "separator":line.separator,"wrap":line.soft_wrap_to_next}),
            )
            .expect("row fingerprint"),
        );
        for segment in &line.styled_segments {
            hash.update(segment.text.as_bytes());
            hash.update(format!("{:?}", segment.style).as_bytes());
        }
    }
    hash.finalize().to_hex().to_string()
}

fn rss() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}
