//! Check exact UTF-8 intervals shared by named task, overview and queue text replies.

use carlos_proto::bytes::ByteOffset;

/// A rounded start must contain the requested byte inside its first complete UTF-8 character.
pub(crate) fn valid_named_range(
    text: &str,
    offset: ByteOffset,
    next: ByteOffset,
    requested: ByteOffset,
    limit: usize,
    complete: bool,
) -> bool {
    valid_utf8_start(requested, offset, text)
        && text.len() <= limit
        && next.get().checked_sub(offset.get()) == Some(text.len() as u64)
        && (complete || !text.is_empty())
}

/// A backward UTF-8 floor must leave the requested byte inside the first returned character.
pub(crate) fn valid_utf8_start(requested: ByteOffset, start: ByteOffset, text: &str) -> bool {
    let Some(skipped) = requested.get().checked_sub(start.get()) else {
        return false;
    };
    skipped == 0
        || text
            .chars()
            .next()
            .is_some_and(|character| skipped < character.len_utf8() as u64)
}
