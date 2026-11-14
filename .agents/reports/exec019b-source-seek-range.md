# EP019b: reject impossible source seek replies

The client now accepts an earlier source position only when the requested byte lies inside
the first complete Unicode character returned there. For example, seeking into a four-byte
emoji may return its first byte. Seeking to byte 5 cannot return ASCII from byte 2.

`SourceWindow::apply` previously allowed any earlier position within three bytes. The new
public Bodies regression fails against that code: it accepts offset 2/text `a` for request 5.
The fix uses the existing private `valid_utf8_start` helper from
`e9f60876d0764ff6e4559c9b09ec652589581bd1`; it preserves the source-specific cursor,
sequence, revision, `has_more` and native completion rules.

The tests reject backward ASCII, a reply ending before the requested position, an incorrect
multibyte start and a shifted empty reply. They also check that failure leaves the read bounds
and original seek request intact. Valid emoji rounding and an empty reply at the exact end
remain accepted. No wire method or product limit changed.

The combined 80-client suite passes at `/tmp/carlos-ep019b/source-seek-range-tests.log`.
The isolated accepted-code snapshot also passes all 75 client tests in the release gate at
`/tmp/carlos-ep019b/markdown-accepted-source-release-checks.log`, excluding the other developer's
unfinished task-fact methods. The failing old-code proof is
`/tmp/carlos-ep019b/source-seek-range-negative.log`.

Root found this while checking the existing source-window API for the platform's future
single pinned document. A caller can copy bounded accepted bytes with `SourceWindow::pieces`,
`start`, `end` and `descriptor`; it needs no new pure-client pass-through API. That producer
is still being proved and is not yet connected to the terminal. The separate Markdown/source
release gate continues; this small report closes only the seek-validator repair.

## Engineering review

The bounded read-only reviewer returned PASS without findings. Raw output follows.

1. Reviewed scope

Reviewed the two-file diff against `e9f60876d0764ff6e4559c9b09ec652589581bd1`, the existing `valid_utf8_start` helper, surrounding source checks, and supplied test logs. Excluded concurrent edits.

2. Findings

No `BLOCKER`, `MAJOR`, or `MINOR` findings.

The helper accepts an exact offset or a backward offset whose first UTF-8 character contains the requested byte. It rejects backward ASCII, ranges ending before or at the requested byte, and shifted empty replies.

The code checks the reply before changing cached state. The new public tests verify that rejected replies preserve both byte bounds and the original seek request. Positive tests retain emoji rounding and exact empty EOF.

The supplied logs show the strengthened test failing against the old code and the restored client suite passing. I did not rerun tests or a build.

3. Corrective guidance

No changes required. Sharing only the UTF-8 start predicate preserves the existing source cursor and completion behavior.

4. Verdict

**PASS**
