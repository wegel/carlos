# EP019b complete borrowed transcript blocks

`RenderInput` now borrows body, path and saved-label text, with role, format and the
label-generated/failure flags. `DisplayMessage`, tool labels, headings, spacing and BlockPlan
consume that view. Core wrappers remain while the terminal caller moves; no Vec<Message>
copy, mutable session or native handle enters the formatter. Stable entry IDs remain in the
transcript/cache; an existing identity test also checks that a compact view borrows its exact
original body after the entry changes.

## Wire fact exposed by the actual consumer

The core previously gave both an absent label and a saved literal label the origin `literal`.
Both can have wire text “Tool output”, but a diff without a saved label needs its derived path
and counts while a deliberately saved label must remain unchanged. `LabelSource::Fallback`
now distinguishes those facts without another string or request method. The client keeps its
saved-label field empty for fallback facts and preserves the existing diff-label rule.
The protocol draft documents this case and the separately readable source of tool calls whose
has_output flag is false.

Two tests use actual exported rows and source chunks. They compare full/compact Gutter/Flow
blocks at 12/28/72/120 columns, including labels, generated descriptions, failures, Markdown,
Unicode, ANSI, diff counts, headings, cells, styles, spacing and soft-wrap flags. Exported
metadata also passes through serde before the client view reads it. Removing only the client
fallback check fails on visible text: “Tool …” replaces the expected “src/é…”. The source is
restored. These comparisons establish agreement between adapters through shared code; the
reviewer separately checked the changed branches against 61b8748, and preserved terminal
screen tests supply separate workflow evidence.

## Checks and checkpoints

Restored workspace tests and strict Clippy pass. The full gate passes 49 daemon, 183 terminal,
23 native-monitor workflows, 67 helper tests, 15 architecture checks and installed Codex
contracts at `target/ui-artifacts/run-U0djcN`. No-default workspace tests and the socket-source
capture/replay also pass. Logs use the prefix `/tmp/carlos-ep019b/render-block-`.

The release perf-session check rotates four binaries for 31 paired rounds on CPU 6 with private
homes and a 160x48 viewport. It uses synthetic seed 1/1000 turns/24 tool lines and the fixed
stale_transcript_replay fixture. Post-exec VmHWM samples every 2 ms measure the actual Carlos
process, avoiding the launcher's inherited wait4 memory floor. Exact commands, binary hashes,
raw samples and paired intervals are in `/tmp/carlos-ep019b/render-block-latency.py` and
`/tmp/carlos-ep019b/render-block-latency/`.

For synthetic, every binary preserves message/row/item counts [6001, 39347, 6000].

- preplan: replay 47.03 ms; scroll/typing/append p95 0.82/0.61/0.65 ms; sampled median VmHWM 23,804 KiB.
- ep019a: replay 46.28 ms; scroll/typing/append p95 0.82/0.61/0.66 ms; sampled median VmHWM 24,784 KiB.
- body: replay 46.56 ms; scroll/typing/append p95 0.82/0.61/0.66 ms; sampled median VmHWM 27,280 KiB.
- current: replay 46.62 ms; scroll/typing/append p95 0.81/0.61/0.66 ms; sampled median VmHWM 24,364 KiB.

For fixture, every binary preserves message/row/item counts [5, 9, 4].

- preplan: replay 1.06 ms; scroll/typing/append p95 0.44/0.39/0.47 ms; sampled median VmHWM 11,400 KiB.
- ep019a: replay 1.00 ms; scroll/typing/append p95 0.44/0.40/0.47 ms; sampled median VmHWM 12,160 KiB.
- body: replay 0.94 ms; scroll/typing/append p95 0.44/0.39/0.47 ms; sampled median VmHWM 14,616 KiB.
- current: replay 1.01 ms; scroll/typing/append p95 0.44/0.39/0.47 ms; sampled median VmHWM 12,032 KiB.

The body reference is the preserved prior release binary from the preceding prototype.
These are embedded-terminal checkpoint figures, not final socket-terminal figures. RSS
includes code/file residency and depends on executable layout and cache state; a lower
sample for one copied reference does not prove a heap saving. The current sample shows no
material draw-latency increase. Keep actual socket/render/RSS comparisons in the final gate.

Release identities:

- preplan: 19,346,328 bytes, SHA-256 `c880c65cc8732b296744bca7f42be5f729ab8b78580c84f2c95113736a1c0b53`, `/tmp/carlos-base/target/release/carlos`.
- ep019a: 20,346,176 bytes, SHA-256 `f03799c8ef130bc5967b6eb9cb24abf3aca2e312d747a4426281ec674711f979`, `/tmp/carlos-ep019b/ep019a-carlos`.
- body: 20,458,632 bytes, SHA-256 `19e3e0fa89dd45f4d0e7edceba349f6d6fc0b9de54ce9c93ba6532e783e30aa6`, `/tmp/carlos-ep019b/borrowed-body-carlos`.
- current: 20,460,048 bytes, SHA-256 `3bc054a3a4a625bdf98f18f8956d0741344292d8a7dfda8ffabd0e18e6ee2560`, `/var/home/wegel/work/perso/carlos-daemon/target/release/carlos`.

## Engineering review

Prompt: `.agents/reviewers/engineering_reviewer.md`, separate bounded read-only session.

Reviewed the diff against `61b8748`, including the old render branches, borrowed inputs, label origins, failure/generated flags, spacing, row counts, and related tests/docs.

No `BLOCKER`, `MAJOR`, or `MINOR` findings. The core adapter preserves the previous branch behavior. The client adapter correctly distinguishes fallback labels from saved literal labels. Both adapters borrow body, path, and saved-label strings without copying them into another `Message`.

Validation: `cargo test --workspace` passed with 1,260 tests passed and 4 ignored. The new whole-block tests passed across the specified widths, themes, and compact/full modes. Those tests compare adapters through shared code; they do not independently prove equivalence with the historic renderer. I checked the changed branches against the base commit directly.

No corrective changes required.

**Verdict: PASS**

The EP remains incomplete. Next connect the Client-backed transcript with one aggregate
source budget, preserve live source/replacement/copy behavior, and port each remaining panel
with its actual wire requests. No product body limit, installed executable, merge or push
was approved here.
