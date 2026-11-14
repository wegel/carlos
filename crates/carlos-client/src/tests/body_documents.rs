//! Prove contiguous source fidelity and aggregate cache pressure across both body formats.

use carlos_proto::output::{OutputRevision, OutputSegment};
use carlos_proto::session::OutputKind;

use super::bodies::{descriptor, output, source};
use super::{entry_id, session};
use crate::{Bodies, BodyToken, ClientError};

fn document(cache: &mut Bodies, row: u64) -> BodyToken {
    cache
        .open_source_document(session(), entry_id(row), OutputRevision::new(3))
        .expect("document lifetime")
}

pub(super) fn fill(cache: &mut Bodies, token: &BodyToken, length: usize) {
    let text = "x".repeat(65536);
    let mut previous = None;
    for offset in (0..length).step_by(65536) {
        let next = format!("at-{}", offset + text.len().min(length - offset));
        let mut chunk = source(
            offset as u64,
            &text[..text.len().min(length - offset)],
            length as u64,
            previous.as_deref(),
            &next,
            offset as u64 + 1,
        );
        chunk.entry = token.entry().clone();
        cache.apply_source(token, chunk).expect("contiguous chunk");
        previous = Some(next);
    }
}

#[test]
fn a_document_borrows_one_complete_source_and_keeps_live_append_revision_guards() {
    let mut cache = Bodies::default();
    let token = document(&mut cache, 10);
    assert!(cache.source(&token).expect("empty").document().is_none());
    fill(&mut cache, &token, 65536 * 9);
    let original = cache
        .source(&token)
        .expect("complete")
        .document()
        .expect("borrow");
    assert_eq!(original.len(), 65536 * 9);
    assert!(original.bytes().all(|byte| byte == b'x'));
    assert_eq!(cache.source(&token).expect("window").pieces().count(), 1);
    let next = descriptor(65536 * 9 + 4);
    cache.observe_source(&session(), &entry_id(10), &next);
    assert!(cache
        .source(&token)
        .expect("appending")
        .document()
        .is_none());
    let mut chunk = source(
        65536 * 9,
        "🌍",
        65536 * 9 + 4,
        Some("at-589824"),
        "appended",
        700000,
    );
    chunk.done = false;
    cache
        .apply_source(&token, chunk.clone())
        .expect("live append");
    assert!(cache
        .source(&token)
        .expect("live")
        .document()
        .expect("complete prefix")
        .ends_with("🌍"));
    assert!(!cache.apply_source(&token, chunk).expect("duplicate"));
    let replacement = cache
        .open_source_document(session(), entry_id(10), OutputRevision::new(4))
        .expect("new revision");
    cache.close(&token);
    assert!(cache.source(&replacement).is_some());
    assert!(cache.source(&token).is_none());
}

#[test]
fn cache_pressure_preserves_exact_cursors_and_releasing_one_format_permits_retry() {
    let mut cache = Bodies::default();
    let large = document(&mut cache, 10);
    fill(&mut cache, &large, 8 * 1024 * 1024 - 65536);
    let normalized = cache
        .open_output(session(), entry_id(11), OutputRevision::new(3), None)
        .expect("normalized lifetime");
    let mut chunk = output(
        None,
        "display",
        vec![OutputSegment {
            kind: OutputKind::Plain,
            text: "y".repeat(65536),
        }],
        50,
        true,
    );
    chunk.entry = entry_id(11);
    cache
        .apply_output(&normalized, chunk)
        .expect("shared allowance edge");
    let extra = document(&mut cache, 12);
    let mut pending = source(0, "🌍", 4, None, "extra", 60);
    pending.entry = entry_id(12);
    let before = cache.request(&extra).expect("original cursor");
    assert!(matches!(
        cache.apply_source(&extra, pending.clone()),
        Err(ClientError::Capacity(_))
    ));
    assert_eq!(cache.request(&extra).expect("unchanged cursor"), before);
    assert!(cache
        .source(&extra)
        .expect("unchanged data")
        .descriptor()
        .is_none());
    cache.close(&normalized);
    cache
        .apply_source(&extra, pending)
        .expect("explicit retry after release");
    assert_eq!(cache.source(&extra).expect("source").document(), Some("🌍"));
    assert_eq!(
        cache
            .source(&large)
            .expect("other document")
            .document()
            .expect("retained")
            .len(),
        8 * 1024 * 1024 - 65536
    );
}

#[test]
fn suffix_updates_that_trim_fit_at_the_aggregate_edge_in_both_formats() {
    let mut cache = Bodies::default();
    let large = document(&mut cache, 10);
    fill(&mut cache, &large, 8 * 1024 * 1024 - 2 * 256 * 1024);
    let raw = cache
        .open_source(session(), entry_id(11), OutputRevision::new(3))
        .expect("suffix");
    fill(&mut cache, &raw, 5 * 65536);
    let normalized = cache
        .open_output(session(), entry_id(12), OutputRevision::new(3), None)
        .expect("normalized suffix");
    let mut previous = None;
    for index in 0..6 {
        let next = format!("output-{index}");
        let mut chunk = output(
            previous.as_deref(),
            &next,
            vec![OutputSegment {
                kind: OutputKind::Plain,
                text: "z".repeat(65536),
            }],
            index + 1,
            false,
        );
        chunk.entry = entry_id(12);
        cache
            .apply_output(&normalized, chunk)
            .expect("trimmed update still fits");
        previous = Some(next);
    }
    assert!(cache.source(&raw).expect("suffix").document().is_none());
    assert!(cache
        .output(&normalized)
        .expect("output")
        .has_missing_prefix());
    assert_eq!(
        cache
            .output(&normalized)
            .expect("output")
            .pieces()
            .map(|p| p.text.len())
            .sum::<usize>(),
        256 * 1024
    );
}

#[test]
fn oversized_documents_fail_locally_while_empty_documents_are_complete() {
    let mut cache = Bodies::default();
    let token = document(&mut cache, 10);
    let too_large = source(0, "x", 8 * 1024 * 1024 + 1, None, "large", 1);
    assert!(matches!(
        cache.apply_source(&token, too_large),
        Err(ClientError::Capacity(_))
    ));
    cache
        .apply_source(&token, source(0, "", 0, None, "empty", 2))
        .expect("empty source");
    assert_eq!(cache.source(&token).expect("empty").document(), Some(""));
}

#[test]
fn a_normalized_chunk_cannot_take_another_documents_budget_or_advance_on_failure() {
    let mut cache = Bodies::default();
    let large = document(&mut cache, 10);
    fill(&mut cache, &large, 8 * 1024 * 1024);
    let display = cache
        .open_output(session(), entry_id(10), OutputRevision::new(3), None)
        .expect("independent output lifetime");
    let chunk = output(
        None,
        "display-next",
        vec![OutputSegment {
            kind: OutputKind::Addition,
            text: "+🌍\n".into(),
        }],
        50000000,
        true,
    );
    let cursor = cache.request(&display).expect("initial output cursor");
    assert!(matches!(
        cache.apply_output(&display, chunk.clone()),
        Err(ClientError::Capacity(_))
    ));
    assert_eq!(
        cache.request(&display).expect("unchanged output cursor"),
        cursor
    );
    assert_eq!(
        cache
            .output(&display)
            .expect("unchanged output")
            .pieces()
            .count(),
        0
    );
    cache.close(&large);
    cache
        .apply_output(&display, chunk)
        .expect("retry after released document");
    assert_eq!(
        cache
            .output(&display)
            .expect("accepted output")
            .pieces()
            .next()
            .expect("output piece")
            .text,
        "+🌍\n"
    );
}
