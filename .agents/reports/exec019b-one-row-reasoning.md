# EP019b: bounded one-row reasoning check

The frame planner only asks whether reasoning occupies exactly one legacy row.
The old predicate counted every wrapped row, which repeatedly scanned long suffixes.
The new predicate preserves its role, width and trimmed-newline guards, then checks
the existing parser's logical lines without counting rows beyond the first hard slice.
It adds no parser, body cap, wrapping choice or source/actor policy.

For one logical line, the helper expands tabs and checks the existing visual width.
If the whole line fits, the old counter returns one immediately. If it overflows and
the first hard slice leaves bytes, the old counter cannot return one: one chosen
word-line would need another hard row, and choosing multiple word-lines already adds
rows. The old counter also restores trailing spaces, so it cannot hide that overflow
by discarding the final spaces. This rejection does not need global optimal wrapping.
Overwide graphemes and rare width interactions that fit a complete hard slice keep
the exact old counter fallback. Those ambiguous cases can still use its global
optimal-fit engine; the helper does not weaken that decision.

The helper reuses markdown_logical_lines, the whole-table branch and the existing
fast_reasoning_summary_plain_text shortcut. It preserves empty-parser fallback and
the old counter's per-raw-line behavior. That shortcut matters: `**hi   **` at width 8
counts as one legacy row but parses to two rendered rows. The new predicate keeps the
old answer. The only owned transcript_wrap.rs edit exposes that existing shortcut
with a brief doc comment; root's Markdown helper extraction stays unchanged.

## Checks and exact source

Six focused tests pass, including 66,666 exhaustive plain-counter comparisons,
80,525 exhaustive Markdown-counter comparisons, 1,000 deterministic mixed fixtures
at six widths, every message role, width below 8, newline guards, trailing spaces,
tables, late references, parser-emitted blank/multiple lines, ligatures, emoji,
zero-width text and an overwide Indic grapheme. Large visible text rejects without
the pathological reference count; small exact fixtures still use that old oracle.

The isolated checkout is `/tmp/carlos-ep019b/reasoning-one-row-checkout`, based on
accepted painter commit `06e265d`, plus the predicate files and root's existing
Markdown logical-line extraction. It has its own target directory,
`/tmp/carlos-ep019b/reasoning-one-row-target`. Its manifest is
`/tmp/carlos-ep019b/reasoning-one-row-evidence/identity.json` and includes that
read-only dependency snapshot explicitly. An earlier ce25616-only snapshot had two
known unused facade exports; accepted 06e265d supplies their production consumer.
No lint was suppressed and no facade or paging helper was changed.

The complete isolated workspace passes 818 main, 83 client and 359 core tests,
plus protocol/daemon/architecture targets. Strict workspace Clippy passes with
`--workspace --all-targets --no-default-features -- -D warnings`.

## Bounded measurements

The ignored probe runs old/new queries in separate release processes on CPU 6.
It records actual source length, BLAKE3, exact Boolean result and elapsed time.
Every old/new pair has the same source hash and answer. Reference sizes stop at
1 MiB; candidate sizes continue through 4/16 MiB. Each figure below is one diagnostic
sample, not a socket latency percentile or a comparison of complete renderers.

- One MiB of ASCII followed by an emoji: 194.32 ms old, 2.47 ms new. New 4/16 MiB
  samples take 7.90/28.37 ms.
- The same text inside the legacy summary delimiters: 182.75 ms old, 0.884 ms new;
  new 4/16 MiB samples take 3.43/14.15 ms.
- One MiB of CJK/emoji/combining/tab words: 55.30 ms old, 14.27 ms new. New 4/16 MiB
  samples take 52.79/208.93 ms; the complete existing Markdown parse remains necessary.
- One overwide Indic grapheme at roughly 1 MiB keeps the old true result through
  the exact fallback: 21.39 ms old, 34.12 ms new. The 16 MiB candidate takes 526.43 ms.
- One MiB of zero-width text keeps the old true result: 2.22 ms old, 2.12 ms new;
  the 16 MiB candidate takes 25.74 ms.

All raw logs and the complete 40-case list are under
`/tmp/carlos-ep019b/reasoning-one-row-evidence/`. The normal release CLI also passes
`perf-session --synthetic --turns 1000 --seed 7 --tool-lines 24 --width 160 --height 48`:
6001 messages, 39358 rows and 6000 items, with 0.72 ms append p95 in that harness smoke.
Its log and separate executable identity are `reasoning-perf-session.log` and
`reasoning-one-row-evidence/cli-identity.json`; this is not a live-owner latency claim.

## Engineering review

The bounded read-only reviewer returned PASS with no findings. It checked the four
scoped files, authorized shortcut visibility/doc change, old-counter dependencies,
source/executable hashes and saved measurements, then reran all six focused tests.
It confirmed all 15 old/new pairs preserve the source hash and Boolean result.
Raw verdict and full log: `/tmp/carlos-ep019b/reasoning-engineering-review.txt`
and `/tmp/carlos-ep019b/reasoning-engineering-review.log`. Root owns the final
source integration; this report approves only the scoped predicate change.
