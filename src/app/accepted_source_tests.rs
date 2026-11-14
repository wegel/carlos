//! Keep a platform-owned prefix exact while the pure cache drops old pieces.

use carlos_client::{Bodies, BodyToken};
use carlos_proto::bytes::ByteOffset;
use carlos_proto::output::OutputRevision;
use carlos_proto::source::SourceChunk;
use serde_json::json;

use super::{SourcePiece, SourcePieceError};

fn opened() -> (Bodies, BodyToken) {
    let mut cache = Bodies::default();
    let token = cache
        .open_source(
            "a".repeat(32).try_into().expect("session"),
            format!("{}-0000000000000001", "b".repeat(32))
                .try_into()
                .expect("entry"),
            OutputRevision::new(3),
        )
        .expect("source");
    (cache, token)
}

fn push(cache: &mut Bodies, token: &BodyToken, offset: usize, text: &str, total: usize) {
    let chunk: SourceChunk = serde_json::from_value(json!({"entry":token.entry(),"offset":offset.to_string(),
        "source":{"revision":"3","bytes":total.to_string()},"text":text,"seq":(offset + 1).to_string(),
        "from":(offset > 0).then(||format!("at-{offset}")),"next":format!("at-{}",offset + text.len()),
        "has_more":offset + text.len() < total,"done":offset + text.len() == total})).expect("source chunk");
    cache.apply_source(token, chunk).expect("validated source");
}

#[test]
fn accepted_pieces_rebuild_a_large_original_after_the_suffix_cache_evicts_its_prefix() {
    let (mut cache, token) = opened();
    let chunk = "\x1b[31m🌍 raw bytes\n".repeat(2000);
    let expected = chunk.repeat(10);
    let mut pinned = String::new();
    for offset in (0..expected.len()).step_by(chunk.len()) {
        push(&mut cache, &token, offset, &chunk, expected.len());
        let next = SourcePiece::from_cache(
            &cache,
            &token,
            OutputRevision::new(3),
            ByteOffset::new(pinned.len() as u64),
        )
        .expect("accepted next interval");
        assert_eq!(next.offset.get(), pinned.len() as u64);
        assert_eq!(next.source.revision, OutputRevision::new(3));
        assert_eq!(next.token.entry(), token.entry());
        assert_eq!(next.token.session(), token.session());
        assert!(next.text.len() <= 65536);
        pinned.push_str(&next.text);
        assert_eq!(next.done, pinned.len() == expected.len());
    }
    assert!(cache.source(&token).expect("bounded source").start().get() > 0);
    assert_eq!(pinned, expected);
}

#[test]
fn a_missing_interval_requests_an_explicit_seek_instead_of_joining_an_incomplete_prefix() {
    let (mut cache, token) = opened();
    for offset in (0..5 * 65536).step_by(65536) {
        push(&mut cache, &token, offset, &"x".repeat(65536), 5 * 65536);
    }
    assert!(matches!(
        SourcePiece::from_cache(&cache, &token, OutputRevision::new(3), ByteOffset::new(0)),
        Err(SourcePieceError::MissingPrefix { .. })
    ));
    assert!(matches!(
        SourcePiece::from_cache(
            &cache,
            &token,
            OutputRevision::new(4),
            ByteOffset::new(65536)
        ),
        Err(SourcePieceError::Replaced)
    ));
    cache.close(&token);
    assert!(matches!(
        SourcePiece::from_cache(
            &cache,
            &token,
            OutputRevision::new(3),
            ByteOffset::new(65536)
        ),
        Err(SourcePieceError::Retired)
    ));
}

#[test]
fn each_copied_piece_ends_on_a_character_boundary_and_rejects_a_partial_start() {
    let (mut cache, token) = opened();
    let first = "x".repeat(65534);
    push(&mut cache, &token, 0, &first, first.len() + 4);
    push(&mut cache, &token, first.len(), "🌍", first.len() + 4);
    let first = SourcePiece::from_cache(&cache, &token, OutputRevision::new(3), ByteOffset::new(0))
        .expect("bounded first");
    assert_eq!(first.text.len(), 65534);
    let second = SourcePiece::from_cache(
        &cache,
        &token,
        OutputRevision::new(3),
        ByteOffset::new(65534),
    )
    .expect("whole emoji");
    assert_eq!(second.text, "🌍");
    assert!(matches!(
        SourcePiece::from_cache(
            &cache,
            &token,
            OutputRevision::new(3),
            ByteOffset::new(65535)
        ),
        Err(SourcePieceError::InvalidOffset)
    ));
}
