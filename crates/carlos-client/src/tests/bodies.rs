//! Keep raw bytes exact while bounding body suffixes and rejecting retired read callbacks.

use carlos_proto::bytes::{ByteLength, ByteOffset};
use carlos_proto::commands::Command;
use carlos_proto::ids::Sequence;
use carlos_proto::output::{OutputChunk, OutputRevision, OutputSegment};
use carlos_proto::session::OutputKind;
use carlos_proto::source::{Source, SourceChunk};

use super::{entry_id, session};
use crate::{Bodies, ClientError};

pub(super) fn descriptor(bytes: u64) -> Source {
    Source {
        revision: OutputRevision::new(3),
        bytes: ByteLength::new(bytes),
    }
}

pub(super) fn source(
    offset: u64,
    text: &str,
    total: u64,
    from: Option<&str>,
    next: &str,
    seq: u64,
) -> SourceChunk {
    let more = offset + (text.len() as u64) < total;
    SourceChunk {
        entry: entry_id(10),
        offset: ByteOffset::new(offset),
        source: descriptor(total),
        from: from.map(|value| value.to_owned().try_into().expect("source cursor")),
        next: next.to_owned().try_into().expect("source cursor"),
        text: text.into(),
        has_more: more,
        done: !more,
        seq: Sequence::new(seq),
    }
}

pub(super) fn output(
    from: Option<&str>,
    next: &str,
    segments: Vec<OutputSegment>,
    seq: u64,
    done: bool,
) -> OutputChunk {
    OutputChunk {
        entry: entry_id(10),
        revision: OutputRevision::new(3),
        from: from.map(|value| value.to_owned().try_into().expect("output cursor")),
        next: next.to_owned().try_into().expect("output cursor"),
        segments,
        has_more: !done,
        done,
        seq: Sequence::new(seq),
    }
}

#[test]
fn source_preserves_split_terminal_controls_and_unicode_without_duplicating_replies() {
    let mut cache = Bodies::default();
    let token = cache
        .open_source(session(), entry_id(10), OutputRevision::new(3))
        .expect("source window");
    assert!(cache.source(&token).expect("source").descriptor().is_none());
    assert!(matches!(
        cache.request(&token).expect("read"),
        Command::Source { cursor: None, .. }
    ));
    let text = "\x1b[31m🌍\x1b[0m";
    let first = source(0, "\x1b[3", text.len() as u64, None, "raw-first", 11);
    assert!(cache
        .apply_source(&token, first.clone())
        .expect("first piece"));
    assert!(!cache.apply_source(&token, first).expect("duplicate"));
    assert!(
        matches!(cache.request(&token).expect("next read"), Command::Source { cursor: Some(cursor), .. } if cursor.as_str() == "raw-first")
    );
    let last = source(
        3,
        "1m🌍\x1b[0m",
        text.len() as u64,
        Some("raw-first"),
        "raw-last",
        12,
    );
    cache
        .apply_source(&token, last)
        .expect("split control remainder");
    let window = cache.source(&token).expect("retained source");
    assert_eq!(window.pieces().collect::<String>(), text);
    assert_eq!(window.start().get(), 0);
    assert_eq!(window.end().get(), text.len() as u64);
    assert!(window.is_done());
    assert!(!window.has_more());
    let wrong = source(0, "different", 100, None, "wrong", 13);
    assert_eq!(
        cache.apply_source(&token, wrong),
        Err(ClientError::InvalidBody)
    );
    assert_eq!(
        cache
            .source(&token)
            .expect("unchanged source")
            .pieces()
            .collect::<String>(),
        text
    );
}

#[test]
fn direct_seeks_accept_only_utf8_rounding_and_opening_another_window_retires_old_callbacks() {
    let mut cache = Bodies::default();
    let old = cache
        .open_source(session(), entry_id(10), OutputRevision::new(3))
        .expect("old read");
    let current = cache
        .seek_source(session(), entry_id(10), descriptor(100), ByteOffset::new(2))
        .expect("seek");
    assert!(
        matches!(cache.request(&current).expect("direct request"), Command::SourceSeek { offset, .. } if offset.get() == 2)
    );
    let rounded = source(0, "🌍", 100, Some("seek-start"), "seek-next", 11);
    assert_eq!(
        cache.apply_source(&old, rounded.clone()),
        Err(ClientError::RetiredBody)
    );
    cache.close(&old);
    cache
        .apply_source(&current, rounded)
        .expect("rounded UTF-8 seek");
    assert_eq!(cache.source(&current).expect("source").start().get(), 0);
    let token = cache
        .seek_source(
            session(),
            entry_id(10),
            descriptor(100),
            ByteOffset::new(10),
        )
        .expect("another seek");
    let wrong = source(6, "🌍", 100, Some("wrong-start"), "wrong-next", 12);
    assert_eq!(
        cache.apply_source(&token, wrong),
        Err(ClientError::InvalidBody)
    );
    assert!(
        matches!(cache.request(&token).expect("unmodified seek"), Command::SourceSeek { offset, .. } if offset.get() == 10)
    );
}

#[test]
fn a_large_source_keeps_a_bounded_exact_suffix_and_reports_the_discarded_byte_interval() {
    let mut cache = Bodies::default();
    let token = cache
        .open_source(session(), entry_id(10), OutputRevision::new(3))
        .expect("source");
    let text = "x".repeat(65536);
    let mut previous = None;
    for index in 0..16 {
        let next = format!("source-{index}");
        cache
            .apply_source(
                &token,
                source(
                    index * 65536,
                    &text,
                    16 * 65536,
                    previous.as_deref(),
                    &next,
                    index + 1,
                ),
            )
            .expect("next source chunk");
        previous = Some(next);
    }
    let window = cache.source(&token).expect("bounded source");
    assert_eq!(window.pieces().map(str::len).sum::<usize>(), 256 * 1024);
    assert_eq!(window.start().get(), 12 * 65536);
    assert_eq!(window.end().get(), 16 * 65536);
    assert!(window
        .pieces()
        .all(|piece| piece.bytes().all(|byte| byte == b'x')));
    cache.observe_source(&session(), &entry_id(10), &descriptor(17 * 65536));
    assert!(cache
        .source(&token)
        .expect("appended descriptor")
        .has_more());
    assert!(!cache.source(&token).expect("appended descriptor").is_done());
    let sought = cache
        .seek_source(
            session(),
            entry_id(10),
            descriptor(17 * 65536),
            ByteOffset::new(0),
        )
        .expect("revisit discarded prefix");
    assert!(
        matches!(cache.request(&sought).expect("direct seek"), Command::SourceSeek { offset, .. } if offset.get() == 0)
    );
}

#[test]
fn normalized_controls_and_classified_pieces_keep_a_separate_cursor_and_retire_on_replacement() {
    let mut cache = Bodies::default();
    let raw = cache
        .open_source(session(), entry_id(10), OutputRevision::new(3))
        .expect("source");
    let display = cache
        .open_output(session(), entry_id(10), OutputRevision::new(3), None)
        .expect("output");
    cache
        .apply_output(&display, output(None, "after-controls", vec![], 11, false))
        .expect("controls advance without text");
    let pieces = vec![OutputSegment {
        kind: OutputKind::Addition,
        text: "+new 🌍\n".into(),
    }];
    let chunk = output(Some("after-controls"), "finished", pieces, 12, true);
    cache
        .apply_output(&display, chunk.clone())
        .expect("display piece");
    assert!(!cache
        .apply_output(&display, chunk)
        .expect("duplicate display chunk"));
    assert_eq!(
        cache
            .output(&display)
            .expect("display")
            .pieces()
            .next()
            .expect("piece")
            .kind,
        OutputKind::Addition
    );
    assert!(cache.output(&display).expect("display").is_done());
    assert!(!cache
        .apply_output(
            &display,
            output(Some("finished"), "finished", vec![], 15, true)
        )
        .expect("same completed output at a newer session sequence"));
    assert!(
        cache.source(&raw).is_some(),
        "normalized reads never consume source requests"
    );
    assert_eq!(
        cache.apply_output(&raw, output(None, "wrong-format", vec![], 13, true)),
        Err(ClientError::InvalidBody)
    );
    cache.observe_revision(&session(), &entry_id(10), OutputRevision::new(4));
    assert!(cache.source(&raw).is_none());
    assert!(cache.output(&display).is_none());
    assert_eq!(cache.request(&display), Err(ClientError::RetiredBody));
}

#[test]
fn output_metadata_and_total_window_counts_stay_bounded_without_evicting_another_live_read() {
    let mut cache = Bodies::default();
    let display = cache
        .open_output(session(), entry_id(10), OutputRevision::new(3), None)
        .expect("output");
    let mut previous = None;
    for index in 0..16 {
        let next = format!("display-{index}");
        let segments = (0..256)
            .map(|index| OutputSegment {
                kind: if index % 2 == 0 {
                    OutputKind::Addition
                } else {
                    OutputKind::Deletion
                },
                text: "x".into(),
            })
            .collect();
        cache
            .apply_output(
                &display,
                output(previous.as_deref(), &next, segments, index + 1, false),
            )
            .expect("bounded display chunks");
        previous = Some(next);
    }
    let window = cache.output(&display).expect("display");
    assert_eq!(window.pieces().count(), 1024);
    assert!(window.has_missing_prefix());
    for index in 11..42 {
        cache
            .open_source(session(), entry_id(index), OutputRevision::new(1))
            .expect("remaining window slots");
    }
    assert!(matches!(
        cache.open_source(session(), entry_id(42), OutputRevision::new(1)),
        Err(ClientError::Capacity(_))
    ));
    assert!(cache.output(&display).is_some());
    cache.close(&display);
    cache
        .open_source(session(), entry_id(42), OutputRevision::new(1))
        .expect("released slot");
}

#[test]
fn empty_completed_source_at_a_newer_session_sequence_does_not_request_a_redraw() {
    let mut cache = Bodies::default();
    let token = cache
        .open_source(session(), entry_id(10), OutputRevision::new(3))
        .expect("source");
    cache
        .apply_source(&token, source(0, "", 0, None, "end", 12))
        .expect("completed empty source");
    assert!(!cache
        .apply_source(&token, source(0, "", 0, Some("end"), "end", 15))
        .expect("unrelated session event changes only the read sequence"));
}
