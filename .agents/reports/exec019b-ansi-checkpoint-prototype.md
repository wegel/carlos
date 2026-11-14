# EP019b: ANSI viewport prototype

This slice tests a smaller renderer; it does not connect that renderer to the terminal.
The new modules compile only in tests. Production code only widens the existing tab helper's
crate-private visibility. The terminal still uses its existing source cache and renderer.
The owner authorized one Sol 6.1 High developer for the separate goal/queue panels; root wrote
this source/renderer slice directly. No Fable review or EP approval belongs to this checkpoint.

## What the prototype proves

`transcript_ansi_pages.rs` keeps the original source and a small list of byte/row/style positions.
It parses ordinary output in batches of at most 128 physical lines or approximately 16 KiB.
A longer physical line stays complete, so the prototype imposes no byte cap on a body or line.
The existing ANSI parser first splits physical CR/LF lines itself; these batch boundaries
therefore preserve its existing treatment of incomplete controls and styles across lines.
A generated SGR prefix and a sentinel on its own next line recover the parser's style state,
including a trailing control that has no visible text.

The row visitor uses the same complete-line textwrap options as the old formatter, including
its choice of natural word breaks. It retains only visible row strings. A borrowed span cursor
preserves the old formatter's exact cell consumption and default-style fallback without copying
the unconsumed suffix after every row. The terminal's old whitespace/style behavior is the
reference here, including its treatment of tabs.

One contiguous styled alphanumeric ASCII word has no natural break to reconsider. Its saved
source range supplies row counts and visible slices directly. Appending alphanumeric bytes to
that exact last source range extends it; a new control, whitespace or other character returns
to the complete parser. Resize can reuse every source/style position and recount ordinary
batches; the saved word uses byte arithmetic.

Six normal Rust tests compare text, styles, colors, widths and wrap flags with the complete
old renderer. They cover8/12/28/80/120 columns, CR/LF, cross-batch resets, incomplete controls,
tabs, CJK, combining marks, emoji joins, empty spans, natural word breaks, trailing spaces,
long lines, live append and resize. A separate row test checks 200 deterministic mixed-text
cases at six widths. A span test compares repeated borrowed reads with the old owned consumer.

## Exact measured artifacts

The isolated checkout is `/tmp/carlos-ep019b/large-probe-checkout`, based on 95d6a73.
All 13 source files used by this prototype match the primary checkout byte-for-byte.
Its Cargo target is separate from the primary checkout. The formatter executable is a
`cargo test --release --no-default-features` artifact, SHA256
`731c23831eee366b7afcfb0703225636110bcc89a1fdd80920e32a985c50c739`.
These formatter functions have no dictation branch. These numbers describe isolated formatter
work with actual validated socket captures, not socket UI latency or a default-feature release.

The ordinary 64 MiB ANSI capture is unchanged from the full-raster baseline report. At 28 columns:

- The old formatter retained 3,195,660 rows and reached 803300 KiB RSS after its first raster.
  Its append/resize p95 samples exceeded 7 seconds.
- The candidate retains 524288 bytes of checkpoint capacity. Its process uses
  74436 KiB RSS after indexing and 74440 KiB with 50 visible rows.
  Those rows own 8862 bytes of Vec/String capacity.
- It indexes in 126.34 ms, draws the tail in 0.183 ms, and measures
  0.643 ms append p95 and 101.82 ms resize p95 across 16 samples each.
- At 120 columns it preserves 798915 rows, draws 50 rows in 0.023 ms, and measures
  0.375 ms append p95 and 242.86 ms resize p95. Initial indexing takes 120.20 ms.

The new actual-daemon capture contains one 64 MiB ASCII word after a red SGR prefix. It arrives
through 1024 bounded source reads and has SHA256
`cbb76345e6d96d7dc3b7d4168f51c034cb1cface04b2feccca072b3598dcc09f`.
Its daemon executable SHA256 is
`090b40b6dbbcf6c1fecc18a1721a8d25df8e6f96c69d6b6c512416a5487651ec`.
The source is a fully received current body in an unfinished turn, as in the earlier capture;
it does not prove a native completed 64 MiB command item.

- Before the saved-word path, the candidate took 1217 ms to draw the tail at 28 columns,
  with 5188 ms append p95. Those rejected results and their executable identity remain as
  `pre-literal-checkpoint-{28,120}.json` and `pre-literal-checkpoint-identity.json`.
- With the saved range it preserves 2,396,745 rows at 28 columns and 559,241 at 120 columns.
  The checkpoint Vec owns 256 bytes; 50 visible rows own 7998/17078 bytes.
- First indexing takes 332.67/300.43 ms. Drawing the tail takes 0.00206/0.00200 ms;
  append p95 measures 0.348/0.338 ms and resize p95 0.0181/0.0209 ms.
- Steady RSS is 73444/73732 KiB. First parse high water remains
  204208/204336 KiB because the upstream owned parser and seeded input temporarily copy the long physical line.

Both captures and both widths copy every original byte through private clipboard shims;
the runner independently checks the saved file's SHA256. Copy takes 85–124 ms in these runs.
This proves the original buffer, not the terminal's Ctrl+Y route. Each benchmark process runs
on CPU 6, and each case runs separately. Sixteen samples within each process give the stated
p95; these are diagnostic samples, not matched multi-round end-to-end comparisons.
Append/resize totals include the small `/proc/self/status` read used by each sample. The
separately recorded initial index and viewport timers exclude that read. No time was subtracted.

The runner assigns a fresh run id and requires the probe to return it with the requested
source size and width. A successful executable that runs no probe must not recycle old JSON.
The direct negative proof accepts `/usr/bin/true` with stale files in the old runner and rejects
it with the current runner. Raw evidence is `/tmp/carlos-ep019b/ansi-stale-metrics-negative.log`.

The JSON/log files are under `target/large-source-probe/ansi/` and
`target/large-source-single-line/ansi/`. Each root retains `checkpoint-identity.json` and
`manifest.json`. The final combined runner output is
`/tmp/carlos-ep019b/ansi-indexed-final-render.log`.

## Checks and remaining work

The isolated full workspace passes with no default features: 794 main tests, 58 client,
347 core and the remaining protocol/daemon/architecture targets; five existing/diagnostic
tests are ignored. All six differential tests also pass in the exact measured release.
Strict workspace Clippy passes, and the repo's size checker finds no function above 60 lines
or file above 400 lines. Primary default-feature and actual terminal checks belong to the
combined state after the queue worker hands back its files; do not substitute these isolated
base-checkout counts for that combined gate.

The unchanged pre-slice reference executable also ran `carlos perf-session --synthetic
--turns 1000 --seed 7 --tool-lines 24 --width 160 --height 48`: 6001 messages/39358 rows/6000 items,
16.25 ms layout and 0.72 ms append p95. This is a harness smoke, not a measurement of the test-only
candidate. Its SHA256 is 869292a589a3d048c36af480821145f7ae10b2419e62a6cc4c52aef5fad1517f;
raw output is `/tmp/carlos-ep019b/ansi-prototype-runtime-reference.log`.

Do not promote this candidate as a complete large-body path yet:

- An ordinary 64 MiB resize still exceeds 100 ms. Run that work away from the input loop.
- A long line with several styles, punctuation, spaces, tabs or Unicode uses the exact
  complete-line visitor. It can still revisit a large prefix; the old width helper can also
  rescan non-ASCII/tab suffixes. It needs a saved general row index before production use.
- The first owned ANSI parse temporarily copies a long line. Measure that cost after the
  general row index exists before choosing a different parser API or backing store.
- Markdown still needs its whole-document semantics and a separate parsed-document memory
  measurement. This ANSI result does not justify a streamed Markdown parser.
- Actual above-cache source ownership, copy/selection, cursor/reset/reconnect behavior and
  terminal input latency remain open. No new pure-client pass-through, file mapping,
  product body cap or generic worker framework was added.

## Engineering review

The first bounded run timed out without a verdict. Its stricter non-interactive retry returned PASS WITH ISSUES, with one MINOR about stale clipboard evidence. The reviewer independently reran all six release tests and checked all four then-current metric files against the report. Raw verdict follows.

1. **Reviewed**

   I reviewed the scoped prototype, its existing helpers, tests, runner scripts and report. I verified the measured executable’s SHA256 and checked all four current metric files against the report. All six differential tests passed when rerun using that exact release executable. I changed no files and ran no large benchmarks.

2. **Findings**

   - **MINOR:** The runner can accept an old clipboard file after a failed copy. [probe-large-format.py:48](/var/home/wegel/work/perso/carlos-daemon/scripts/probe-large-format.py:48) reuses a filename, and line 59 checks its digest without first removing it. The Rust copy helper records the backend result without requiring success. A later probe could therefore return fresh metrics while the runner validates bytes saved by an earlier run. The fresh metric run ID correctly rejects stale JSON, but does not establish that this run wrote the clipboard file.

   I found no BLOCKER or MAJOR defects in the scoped Rust prototype. The report accurately limits its claims to formatter measurements and explicitly identifies the remaining long-line, memory and terminal work.

3. **Corrective guidance**

   Include the fresh run ID in the clipboard filename, or remove the prior file before starting the probe and require a newly created file afterward. Also require the Rust copy helper to report a successful backend. Add a small runner check where fresh metrics accompany a failed copy and an existing matching clipboard file; the runner should reject it.

4. **Verdict**

   **PASS WITH ISSUES** for this test-only checkpoint. The current artifacts support the report’s narrowly stated performance claims. The minor issue weakens repeatable copy verification; it does not demonstrate that the recorded copies failed.

Root fixed the MINOR: every copy now uses a new run-specific path, the Rust probe requires a successful clipboard program, and the runner verifies that same path and backend before its digest. Fresh metrics plus either a missing new copy or a failed program now fail in the direct negative proof at `/tmp/carlos-ep019b/ansi-stale-clipboard-negative.log`. All four actual captures/widths were rerun with this guard; the current source/executable digest map is `target/large-source-probe/prototype-source-identity.json`. The narrow follow-up returned PASS without findings; final strict Clippy also passes. Its raw verdict follows.

1. **Reviewed**

   I reviewed only `run_case`, `copy_original`, the supplied negative-check script and log, and the four saved clipboard artifacts. I changed no files and ran no benchmarks.

2. **Findings**

   No BLOCKER, MAJOR, or MINOR findings remain within this repair.

   The runner assigns each copy a UUID filename, requires fresh metrics with the expected path and successful backend, and checks the file’s digest. Rust rejects any backend other than `ClipboardBackend::Program`.

   The negative evidence shows that the runner rejects both fresh metrics without a newly written file and metrics reporting a failed program backend, despite an existing matching clipboard file.

   All four saved metrics name distinct files containing the run UUID. Both executable records match the supplied SHA256.

3. **Corrective guidance**

   No further changes are required for the previously reported stale-file issue. I did not independently verify the pending Clippy result.

4. **Verdict**

   **PASS** for this bounded repair. No new Fable review or EP approval occurred.

After the review, only private API doc comments changed. Rebuilt the isolated release, reran all four capture/width cases and refreshed the exact source/executable digest map; the formatter algorithm and copy repair stayed the same.
