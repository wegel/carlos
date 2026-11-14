# EP019b borrowed-source renderer prototype

This is a measured prerequisite for the terminal port, not a completed client or cache.

## Code and observed behavior

`BodyInput` borrows role, format, file path and one complete source string. The existing
BlockBody count/render methods now delegate to the same functions through this view. No
production socket path or larger body cache was added. The client constructor remains
test-only while the caller is being built.

Five unit tests cover Markdown tables/references/fences, reasoning, Unicode wrapping,
ANSI colors, diffs, source highlighting, user steering and tool calls at 28/72/120 columns.
They compare text, styles, cell counts and soft-wrap flags; pointer checks confirm that the
adapter borrows the original string. A suffix with identical visible tail text loses ANSI
color state. Missing source, stale descriptors and stale tool revisions are rejected.

Tool rows can have empty inline text and no Entry.source descriptor. Their tool.output_revision
fences the separately read body. Tool calls can have has_output=false while still owning
source. The final test uses an actual core-exported tool call and real source_chunk rather
than constructing a more convenient wire shape.

## Actual socket capture and replay

Run `target/ui-venv/bin/python scripts/probe-client-render.py`, then
`CARLOS_RENDER_PROBE=target/render-probe cargo test --bin carlos socket_render_probe -- --ignored --nocapture`.
The script starts an isolated carlosd and deterministic native Codex peer, with no model call
or account. It compares every original source byte with the expected stored core body.
The command body includes the existing command heading and exit footer. The Rust probe
then streams those captured SourceChunks through real carlos-client Bodies and one contiguous
String candidate. It releases the comparison window before rendering the candidate.

- markdown: 552059 bytes in 9 reads; captured Python socket/read/log work 9.64 ms; SHA-256 `4a8de93c6b66218f44d026537705697eba74fce2b6846dc688748ee3027a6bd4`.
- ansi: 456047 bytes in 7 reads; captured Python socket/read/log work 7.18 ms; SHA-256 `7fb6b48621240cc3fb913a4277c0c71151615f15fc9797c6e18c3105d136110b`.

Every source reply is at most 64 KiB. The existing 256 KiB client window discards a prefix for
both cases; the complete-document constructor correctly refuses that suffix. The contiguous
candidate preserves all bytes and agrees with the core-message adapter at 28/120 columns.
Fingerprints cover text, styles, widths, roles and wrap flags. Both adapters use the same
refactored renderer, so this is adapter agreement, not an independent historic-renderer proof.
The preserved terminal screens and matched release executable checks provide separate evidence.

The first debug replay reassembles the captured source in 0.95/0.65 ms. Count plus full-body
render is 29-38 ms across the four old/candidate comparisons. This includes fingerprint work
and a full 24,000-row body, not only visible terminal rows. One process retained allocator
pages between cases: RSS was 11.4 MiB before the first source, 13.2 MiB after source and 25.2 MiB
with rendered rows. Those samples do not establish the final product cache bound or peak RSS.
Raw evidence: /tmp/carlos-ep019b/render-probe-{capture.json,result.log}; durable generated
artifacts: target/render-probe/{manifest.json,markdown,ansi,daemon}.

## Limits and next work

The probe has an 8 MiB fixture limit only. No product body limit was accepted. It does not
prove live append/replacement while rendering, end-to-end copy/selection, main terminal
startup, complete-block spacing, private drafts, or viewport cache behavior. The terminal
still owns an embedded core session. Next build its first socket-driven transcript consumer
using borrowed views, choose and measure one aggregate source-cache policy, then port each
panel with its real typed requests. Do not widen older task/history APIs ahead of that work.

## Engineering review

Reviewer prompt: `.agents/reviewers/engineering_reviewer.md`. Both bounded sessions were
read-only. The first finding was fixed before the narrow passing follow-up.

### Initial review

Reviewed the scoped diff against `99d39aa`, including borrowed lifetimes, count/render branches, client mappings, and socket replay. The four focused tests and captured socket replay pass. I found no accidental production count/render change or text copy.

- **MAJOR: The client constructor silently discards tool-call source.** At [transcript_input.rs:41](/var/home/wegel/work/perso/carlos-daemon/src/app/transcript_input.rs:41), the constructor requires external source only when `tool.has_output` is true. The core exports ordinary tool calls with empty `entry.text`, no `entry.source`, and `has_output: false`. Consequently, even when the caller supplies complete source such as `"Bash\nprintf 'hello'\n"`, the constructor ignores it and returns an empty body. It also accepts missing source as an empty body. The current tool-call test misses this because its helper supplies a source descriptor that real tool-call rows lack.

  **Corrective guidance:** Handle tool-call source explicitly, checking the tool revision and complete byte length before borrowing it. Reject missing source when the tool-call body must be fetched. Add a test using the actual exported row shape and compare counts, text, styles, and wraps at the stated widths.

The socket replay supports the reported large Markdown/ANSI cases and suffix rejection. Both renderer paths use the refactored helpers, so their equal fingerprints establish agreement between the adapters, not an independent comparison with the renderer at `99d39aa`. They also do not establish terminal cutover or release performance.

**Verdict: FAIL**

### Corrected follow-up

Reviewed `BodyInput::from_client` and the new test using a core-published tool-call row.

Findings: no BLOCKER, MAJOR, or MINOR issues in this follow-up. The constructor now requires source for tool rows even when `has_output=false` and no descriptor exists. That path checks the tool’s output revision and exact source byte count.

The new test reproduces the previously broken row shape, rejects missing source, and compares nonzero counts plus complete rendered text, styles, and wraps at 28/72/120 columns against the original core message. I ran that test; it passed.

Corrective guidance: none required for the prior finding.

**PASS**. The prior MAJOR is resolved.

## Release checkpoint and checks

Default and no-default workspace tests, strict Clippy and the repeatable runner pass. The
runner covers49daemon and183terminal workflows, 67 helper tests and 15 architecture checks at
`target/ui-artifacts/run-0UU3za`. The earlier stop gate separately passed installed Codex
contracts and 23 native-monitor workflows at run-PW0kwe. The resize-only fixture fix is 287a0ac.

The 31-round rotating paired perf-session check uses private homes, CPU 6 and 160x48 cells.
Each binary runs `perf-session --synthetic --turns 1000 --seed 1 --tool-lines 24`, and the same
`src/tests/fixtures/stale_transcript_replay.jsonl` fixture. Exact commands and the sampling script are in
`/tmp/carlos-ep019b/render-latency.py`. The samples ran after builds and terminal checks.

For synthetic, all binaries retain identical message/row/item counts [6001, 39347, 6000].

- preplan: replay 46.94ms; scroll/typing/append p95 0.82/0.61/0.65ms; median sampled process high-water RSS 23,732KiB.
- ep019a: replay 46.64ms; scroll/typing/append p95 0.82/0.61/0.66ms; median sampled process high-water RSS 24,844KiB.
- current: replay 46.87ms; scroll/typing/append p95 0.82/0.62/0.67ms; median sampled process high-water RSS 24,436KiB.

For fixture, all binaries retain identical message/row/item counts [5, 9, 4].

- preplan: replay 1.05ms; scroll/typing/append p95 0.44/0.42/0.47ms; median sampled process high-water RSS 11,296KiB.
- ep019a: replay 0.99ms; scroll/typing/append p95 0.44/0.41/0.47ms; median sampled process high-water RSS 12,260KiB.
- current: replay 1.00ms; scroll/typing/append p95 0.45/0.43/0.49ms; median sampled process high-water RSS 12,104KiB.

CPU 6 was pinned for every process. RSS comes from /proc/PID/status VmHWM sampled every
2 ms after /proc/PID/exe identifies the actual Carlos executable. The built-in RSS reader
returns 0 in all retained baselines. A first wait4-only run also inherited the Python launcher’s
higher memory floor, reporting the same 38 MiB for all binaries; that is not application RSS.
The final sampler records both numbers in sampling.jsonl and uses only post-exec VmHWM.
Earlier concurrent-build/wait4-floor measurements remain separately under
`render-latency-wait4-floor` and do not support the figures above.

The synthetic paired median append difference is 0.01 ms against the original and 0.00 ms
against EP019a. Scroll p95 differs by 0.00 ms against both; typing differs by 0.00 ms when
paired by round despite the rounded medians above. Current synthetic memory is about 0.7 MiB
above the original and 0.4 MiB below EP019a. These are checkpoint embedded-terminal figures,
not final socket-terminal performance. The source replay separately proves adapter agreement.

Release executable identities:

- preplan: 19,346,328bytes; SHA-256 `c880c65cc8732b296744bca7f42be5f729ab8b78580c84f2c95113736a1c0b53`; `/tmp/carlos-base/target/release/carlos`.
- ep019a: 20,346,176bytes; SHA-256 `f03799c8ef130bc5967b6eb9cb24abf3aca2e312d747a4426281ec674711f979`; `/tmp/carlos-ep019b/ep019a-carlos`.
- current: 20,458,632bytes; SHA-256 `19e3e0fa89dd45f4d0e7edceba349f6d6fc0b9de54ce9c93ba6532e783e30aa6`; `/var/home/wegel/work/perso/carlos-daemon/target/release/carlos`.

The release pair was built but not installed. Main and ~/.local/bin/carlos are unchanged.
