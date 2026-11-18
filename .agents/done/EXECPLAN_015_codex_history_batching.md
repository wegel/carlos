# Codex large-session resume

## Goal

Load the reported long Codex session quickly through public native APIs, without
changing its files, discarding history or extending the 20-second deadline.

## Progress

- Parent approved the reader/transport design after native measurements.
- Replaced primary one-item calls with full-turn pages of up to 100 turns.
- Kept existing projection hydration, older full-read fallback and legacy-only
  recovery. Modern paginated history stays authoritative.
- Added one correlated large history reply allowance for stdio and Unix sockets.
  Ordinary replies/events/writes and the charged UI stream keep their old limits.
- Added full-array/ID/count checks, local-write grant cleanup and short reconnect
  errors after parent review. Focused terminal/native and Rust checks pass.
- Parent review passed. The verified release is installed as regular 0755 files;
  the actual installed copy reaches rendered readiness in 1.13 seconds.
- All release, no-default-feature, additional socket and installed-file checks pass.

## Surprises & Discoveries

The reported target file contains38,075,963 bytes. Its byte-exact ancestor chain
contains seven files, including a207MB ancestor. Native0.167.1 projects40 visible
turns and2,303 items into about5.42MB. The raw file sizes do not describe the
canonical visible history. Read-only copies stayed private in temporary folders;
no model requests or real-session resume were made.

On an isolated exact copy, the old reader made2,303 item RPCs in2.831 seconds.
Thread-wide100-item pages took0.107 seconds/24 RPCs. Full100-turn pages took
0.177 seconds/one RPC with the same canonical turn/item identities. Complete
installed-Carlos reached its final metadata request in3.762 seconds; the changed
debug Carlos reached that same request fence in1.108 seconds. The reviewed
release package then reached returned final metadata plus a rendered ready UI
with actual native transcript text in1.179 seconds. The later proof asserts the
chosen `/proc` executable and exact40-turn/2,303-item contents. The user's real-home20-second failure was not reproduced in this
small isolated home. The confirmed defect is unnecessary per-record native
lineage/database work, which multiplies any per-request cost.

Private numeric/wire evidence:

- `/tmp/carlos-history-cost-6htjb9e2/report.json`: complete traversal comparison.
- `/tmp/carlos-history-cost-fo1pwnfx/report.json`: full-turn100 comparison.
- `/tmp/carlos-history-ui-7pp5wix9/report.json`: installed baseline startup.
- `/tmp/carlos-history-ui-d5o3zsrb/report.json`: changed complete startup.

The synthetic native fork proof loads31 turns/1,711 items in one12,811,587-byte
reply, including compaction and two large answers, in0.864 seconds. It verifies
exact canonical turn IDs, excludes the ancestor's later tail and keeps rewind
anchors. Its source and model endpoint belong only to the test.

## Decision Log

- Use the public full-turn reader because it supplies a turn and its items
  together. This avoids joining separate snapshots when another client starts
  a new turn. Preserve the existing item reader only for older projections.
- Keep the64MiB aggregate cap and20-second deadline. A single blocking history
  page can use64MiB off the UI stream. Physical stdio/WS decoding has that hard
  ceiling; exact response IDs gate acceptance above8MiB. Unknown/unrelated large
  frames fail rather than enter the UI queue. Writes/metadata/async events stay8MiB.
- Permit one active large read and at most64 unanswered timed-out read IDs.
  Successful/late replies release their exact grant. New history reads fail at
  that retained-ID cap rather than grow memory or forget a legitimate late reply.
- Validate full and legacy item arrays before installing a transcript, including
  nonempty IDs and the existing500,000-record bound. Deduplicate within a turn,
  never across turns.
- Shorten reconnect diagnostics so large native errors cannot exceed the reserved
  terminal frame or bury the useful reason below pages of error body.

## Validation

- Focused Rust history/transport checks:68 passed before the final one-line
  diagnostic regression was added. Strict Clippy and all-file style checks passed.
- Focused native tests:3 passed at `/tmp/carlos-history-native-focused`, including
  old legacy recovery, dense history over Unix WS and the large fork/ancestor cut.
- An initial large-error UI check correctly reached offline state, but its8KiB
  displayed body pushed the reason above the screen. Preserved at
  `/tmp/carlos-history-ui-focused`; the displayed diagnostic was then reduced.
- A second negative UI check exposed raw newlines in a short native message;
  Carlos now collapses whitespace and replaces terminal controls while building
  its bounded output. A later fixture assertion needed to join visibly wrapped
  words. Both failed screens remain in `/tmp/carlos-history-ui-final` and
  `/tmp/carlos-history-ui-bounded-final`.
- Final focused full-page/error UI checks:2 passed at
  `/tmp/carlos-history-ui-final-pass`. No source edits followed that check.
- Final combined release gate: `bash scripts/test.sh --release --codex-contract
  --claude-contract` PASS. Artifacts `target/ui-artifacts/run-eZKGPJ`, log
  `/tmp/carlos-history-final-release.log`. Results:662 Rust passed/1 ignored,
  52 source plus52 standalone helper tests with284 assertions each,157 terminal
  workflows, public Codex contracts,22 native Codex cases and7 native Claude
  cases. Package smoke, recorder cleanup, fmt and strict Clippy pass.
- The release native fork/large-page case took0.612 seconds; its public JSON
  projection contains12,811,587 bytes,31 turns and1,711 items. The shared socket
  dense fixture took0.193 seconds with zero per-item history calls.
- Reusable private-copy driver: `/tmp/carlos-history-copy-ui.py`. Run with
  `target/ui-venv/bin/python /tmp/carlos-history-copy-ui.py --binary PATH`.
  It copies the preserved isolated fixture again, forces its native resume cwd
  and provider into that private root, uses a fake key and blocked loopback model
  endpoint, and never resumes the real source. It waits for final metadata's
  reply and rendered ready/native transcript text, asserts exact counts, no
  model-input RPCs and `/proc` executable identity. Final release proof:
  `/tmp/carlos-history-copy-ui-1hgx0tgj/report.json`,1.179 seconds.
- An earlier driver stopped at the final metadata request; its timing is retained
  as preliminary evidence only. Parent requested and reviewed the stronger
  returned-metadata/rendered-UI fence before handoff.

Read-only source verification found all six ancestor hashes unchanged. The live
user target gained176,757 bytes/76 records while this work ran, spanning one
complete native turn from14:04:38 to14:07:00Z. Its original38,075,963 bytes remain
an exact prefix. All our probes used their own `CODEX_HOME`, copied files,
explicit isolated cwd and local/blocked model endpoints. We neither resumed nor
wrote the real session. Evidence stays private in
`/tmp/carlos-history-cost-6htjb9e2/original-target-prefix.json`.

## Outcomes & Retrospective

The tested release package is `target/ui-artifacts/run-eZKGPJ/package`.
Carlos SHA256: `7b4e50ecee94edfde08fb66ada6d464cd73653799271c0cd72952661226da4fa`.
The unchanged helper SHA256 is
`a44f4b3e02aeeac1f314eabd1c8ad1aac06b291a204fbd342d35944ac7c7311a`.

Parent's production source and release-gate review PASS. Parent found one test
recorder mismatch after that review: its upstream native WebSocket receive limit
was still8MiB. Only that upstream allowance now matches the64MiB physical ceiling;
the downstream client-request limit remains8MiB. The native fixture launcher was
split so the same generated fork can run over either transport.

Additional fixture-only checks against the unchanged `run-eZKGPJ` package:

- Two large ancestry cases PASS at `/tmp/carlos-history-native-both-transports`:
  stdio0.610 seconds; Unix WebSocket0.552 seconds. Both return one12,811,587-byte
  public JSON projection with31 turns/1,711 items, retain the exact ancestor cut,
  keep rewind anchors and make no model calls. Carlos exit leaves the shared
  native server alive until the fixture explicitly cleans it up.
- Six existing socket/reconnect cases PASS at
  `/tmp/carlos-history-socket-reconnect-focused`, covering running-turn recovery,
  root/child request replay scope, initial active attach, abandoned owned-turn
  rejection and dense history.
- The recorder's bounded shutdown test and whitespace check PASS.

These fixture-only follow-ups did not rebuild or alter the release package; the
full gate above still ran22 native Codex cases, and the added socket ancestry
case passed separately. No install or commit has been made for this fix. The exact real-home
20-second timeout was not reproduced, so these probes establish the unnecessary
request amplification and corrected full-copy load time, not a claimed universal
latency guarantee for every native database or filesystem.


## Parent review and installed result

The parent engineering review gives PASS. Carlos now requests full-turn pages
under the existing 20-second deadline. The parent inspected exact-ID large-read
grants, late-reply drains, normal event/write limits, full/legacy item checks,
error handling and atomic history replacement. The agent corrected all findings.
No further product changes followed the frozen release gate.

Run `target/ui-artifacts/run-eZKGPJ` passed 662 Rust tests/one ignored,
52 source plus 52 standalone helper checks, all 157 terminal workflows,
22 native Codex and seven native Claude cases, public/package contracts and
strict checks. The fixture-only socket-recorder correction then passed the same
12.8 MiB ancestry case over stdio and native Unix WebSocket, plus six existing
socket cases and recorder cleanup. A separate build without default features
passed after the release suite. UI draw benchmarks remained below one ms:
scroll p95 0.58 ms, typing p95 0.28 ms, working p95 0.27 ms and append p95 0.30 ms.
The report is `/tmp/carlos-history-final-perf.txt`.

The parent verified all 503 package checksums and installed Carlos as a regular
0755 file at `~/.local/bin/carlos`, SHA256
`7b4e50ecee94edfde08fb66ada6d464cd73653799271c0cd72952661226da4fa`.
The compatible helper remains a regular 0755 file with its unchanged hash.
The full package and notices are kept under
`~/.local/lib/carlos/releases/0.1.0-codex-refresh-7b4e50ecee94`; the previous
pair and metadata are backed up under
`~/.local/lib/carlos/backups/carlos-before-refresh-20261111T142554Z`.

The actual installed executable loaded the preserved private session copy to
returned metadata and a rendered ready transcript in 1.1268 seconds. It read
all 40 turns and 2,303 items with one full-turn history request and no per-item
requests. The driver checked the process executable and hash, asserted no model
input, and used only copied histories and an isolated working directory.
Evidence: `/tmp/carlos-history-copy-ui-96w1iqmy/report.json` and
`/tmp/carlos-history-installed-copy.log`. The original real-home timeout was
not reproduced in the small isolated home; the excessive RPC path was removed
and the complete copied history now loads near the native TUI benchmark.

Four further installed-file checks passed: large forked history over stdio and
WebSocket, shared reconnect, and Claude context. The driver checked actual
installed Carlos/helper/native executable paths with isolated homes and local
model fixtures. Evidence: `/tmp/carlos-refresh-installed-isf3ym6i` and
`/tmp/carlos-history-installed-smoke.log`. Install metadata and history record
all checks, hashes, backup and native versions. The native executables remain
unchanged. All probes left the original sessions alone; the live target gained
an external turn while its earlier byte prefix and all ancestor hashes stayed
intact. No commit was made. Existing Carlos processes must restart to use the
new executable.
