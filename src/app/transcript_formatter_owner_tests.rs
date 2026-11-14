//! Feed accepted cache pieces through the bounded formatter and reject reused-token completions.

use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};

use carlos_client::{Bodies, BodyToken};
use carlos_core::session::models::{MessageKind, Role};
use carlos_proto::output::OutputRevision;
use carlos_proto::source::SourceChunk;
use serde_json::json;

use super::{Formatter, Reply, Request, Transfer};
use crate::app::layout_state::TranscriptTheme;
use crate::app::render_entry_cache::RowFormat;
use crate::app::transcript_input::RenderInput;

fn cache() -> (Bodies, BodyToken) {
    let mut cache = Bodies::default();
    let token = cache
        .open_source(
            "a".repeat(32).parse().expect("session"),
            format!("{}-0000000000000001", "b".repeat(32))
                .parse()
                .expect("entry"),
            OutputRevision::new(3),
        )
        .expect("source");
    (cache, token)
}

fn push(cache: &mut Bodies, token: &BodyToken, offset: usize, text: &str, total: usize) {
    let chunk: SourceChunk = serde_json::from_value(json!({"entry":token.entry(),"offset":offset.to_string(),
        "source":{"revision":"3","bytes":total.to_string()},"text":text,"from":(offset>0).then(||format!("at-{offset}")),
        "next":format!("at-{}",offset+text.len()),"seq":(offset+1).to_string(),"done":false,
        "has_more":offset+text.len()<total})).expect("wire piece");
    cache
        .apply_source(token, chunk)
        .expect("accepted cursor/revision/offset");
}

fn show(formatter: &mut Formatter, first: usize) -> u64 {
    let facts = RenderInput {
        role: Role::ToolOutput,
        kind: MessageKind::Plain,
        text: "",
        file_path: None,
        label: None,
        label_generated: false,
        failed: false,
    };
    formatter
        .show(
            facts,
            RowFormat {
                width: 28,
                compact: false,
                theme: TranscriptTheme::Gutter,
                hidden: false,
            },
            None,
            first,
            8,
        )
        .expect("viewport intent")
}

fn feed(formatter: &mut Formatter, cache: &Bodies, end: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while formatter.offset().expect("active source").get() != end as u64 {
        let transfer = formatter.feed(cache).expect("bounded accepted transfer");
        assert!(
            matches!(transfer, Transfer::Queued | Transfer::Busy)
                || formatter.offset().expect("admitted pending piece").get() == end as u64
        );
        assert!(
            Instant::now() < deadline,
            "worker must drain bounded pieces"
        );
        thread::yield_now();
    }
}

fn reply(formatter: &mut Formatter) -> Reply {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut replies = formatter.poll();
        if !replies.is_empty() {
            let reply = replies.remove(0);
            if let Reply::Failed { error, .. } = &reply {
                panic!("formatter failed: {error:?}");
            }
            return reply;
        }
        assert!(Instant::now() < deadline, "worker reply");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn the_worker_keeps_exact_original_bytes_after_the_pure_cache_discards_old_pieces() {
    let (mut cache, token) = cache();
    let mut formatter = Formatter::new().expect("worker");
    let job = formatter
        .open(token.clone(), OutputRevision::new(3))
        .expect("source job");
    let view = show(&mut formatter, 0);
    let chunk = "\x1b[31mred 🌍 line\x1b[0m\n".repeat(1000);
    let expected = blake3::hash(chunk.repeat(16).as_bytes());
    for index in 0..16 {
        push(
            &mut cache,
            &token,
            index * chunk.len(),
            &chunk,
            16 * chunk.len(),
        );
        feed(&mut formatter, &cache, (index + 1) * chunk.len());
    }
    assert!(cache.source(&token).expect("suffix cache").start().get() > 0);
    let Reply::Frame {
        job: got,
        view: shown,
        source,
        prepared,
    } = reply(&mut formatter)
    else {
        panic!("frame")
    };
    assert_eq!((got, shown), (job, view));
    assert_eq!(source.bytes.get(), (16 * chunk.len()) as u64);
    let lines = prepared.lines.expect("bounded visible rows");
    assert!(lines.retained().count() <= 9);
    assert!(prepared.count > 10_000);
    assert_eq!(formatter.feed(&cache), Ok(Transfer::CaughtUp));
    formatter
        .sender
        .send(Request::Digest(job))
        .expect("diagnostic source hash");
    let Reply::Digest { job: got, digest } = reply(&mut formatter) else {
        panic!("digest")
    };
    assert_eq!((got, digest), (job, expected));
    let next = show(&mut formatter, 10_000);
    let Reply::Frame { view, prepared, .. } = reply(&mut formatter) else {
        panic!("later viewport")
    };
    assert_eq!(view, next);
    assert!(prepared.lines.expect("later rows").get(10_000).is_some());
    formatter.retire();
    assert!(formatter.offset().is_none());
}

#[test]
fn a_restarted_cache_can_reuse_a_token_without_reviving_an_old_formatted_source() {
    let (mut old, old_token) = cache();
    let mut formatter = Formatter::new().expect("worker");
    let first = formatter
        .open(old_token.clone(), OutputRevision::new(3))
        .expect("first job");
    show(&mut formatter, 0);
    push(&mut old, &old_token, 0, "old source", 10);
    feed(&mut formatter, &old, 10);
    barrier(&formatter);
    // The old frame is now queued, so this checks the receiver's fence, not just owner cancellation.
    let (mut current, token) = cache();
    assert_eq!(
        token, old_token,
        "the new pure cache starts its counter again"
    );
    let second = formatter
        .open(token.clone(), OutputRevision::new(3))
        .expect("new epoch");
    assert_ne!(first, second);
    show(&mut formatter, 0);
    push(&mut current, &token, 0, "new source", 10);
    feed(&mut formatter, &current, 10);
    let Reply::Frame { job, prepared, .. } = reply(&mut formatter) else {
        panic!("fresh frame")
    };
    assert_eq!(job, second);
    let rows = prepared.lines.expect("fresh rows");
    assert!(rows.retained().any(|line| line.text.contains("new source")));
    assert!(!rows.retained().any(|line| line.text.contains("old source")));
}

fn barrier(formatter: &Formatter) {
    let (sent, received) = channel();
    formatter
        .sender
        .send(Request::Barrier(sent))
        .expect("ordered barrier");
    received
        .recv_timeout(Duration::from_secs(5))
        .expect("older frame queued before barrier");
}

#[test]
fn retiring_a_window_discards_its_already_queued_frame() {
    let (mut cache, token) = cache();
    let mut formatter = Formatter::new().expect("worker");
    formatter
        .open(token.clone(), OutputRevision::new(3))
        .expect("source");
    show(&mut formatter, 0);
    push(&mut cache, &token, 0, "body", 4);
    feed(&mut formatter, &cache, 4);
    barrier(&formatter);
    cache.close(&token);
    assert!(matches!(
        formatter.feed(&cache),
        Err(super::TransferError::Retired)
    ));
    formatter.retire();
    assert!(formatter.poll().is_empty());
}

#[test]
fn polling_delivers_the_latest_view_after_backpressure_without_new_source_bytes() {
    let (mut cache, token) = cache();
    let mut formatter = Formatter::new().expect("worker");
    formatter
        .open(token.clone(), OutputRevision::new(3))
        .expect("job");
    show(&mut formatter, 0);
    let text = "body\n".repeat(50);
    push(&mut cache, &token, 0, &text, text.len());
    feed(&mut formatter, &cache, text.len());
    let Reply::Frame { .. } = reply(&mut formatter) else {
        panic!("initial frame")
    };
    let (entered, held) = channel();
    let (release, resumed) = channel();
    formatter
        .sender
        .send(Request::Hold(entered, resumed))
        .expect("hold owner");
    held.recv_timeout(Duration::from_secs(5))
        .expect("owner held");
    for first in 1..=4 {
        show(&mut formatter, first);
    }
    let latest = show(&mut formatter, 5);
    assert!(!formatter.producer.as_ref().expect("producer").view_sent);
    release.send(()).expect("resume owner");
    let Reply::Frame { view, prepared, .. } = reply(&mut formatter) else {
        panic!("latest viewport frame")
    };
    assert_eq!(view, latest);
    assert!(prepared.lines.expect("viewport rows").get(5).is_some());
}
