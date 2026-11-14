# EP019a: working local daemon, ready for owner review

Completed on `iphone-spike` in `/var/home/wegel/work/perso/carlos-daemon`,
2026-11-14. The daemon supports real Codex, Claude and Grok adapters through a
private Unix socket. Two clients can read the same session, send ordered text,
watch different tool output, reconnect and race approval answers. A held native
send reply does not block reads, interrupts or a valid answer. Disconnecting a
client leaves its session running.

The terminal still owns its session in process. EP019b will add all remaining
typed terminal commands, route the terminal through a shared client, and delete
the embedded path in a separate buildable commit. The owner-review gate is now
reached; EP019b has not begun. No spike binary was installed, merged or pushed.

## What changed

`carlos-core::engine` gives each live session one owner thread. Workers receive
captured native handles rather than a mutable session or whole backend client.
The owner queues ordinary commands in order and claims exact live turns/cards
before assigning independent control workers. All three native writers check
permission again before starting bytes. Late completions verify the original
conversation before changing state. Grok now reads large startup histories
asynchronously through a bounded queue, with an explicit history-adopted fact
instead of a guessed delay; both terminal and daemon use that path.

`crates/carlosd` adds a manager, private same-user Unix listener, bounded socket
workers and a small SQLite catalogue. Saved descriptions stay dormant until
subscribed, and concurrent subscribers share one restore. Startup cards remain
answerable before history is ready. The daemon saves native identity before
accepting create; it never guesses a new native session after restore failure.
Saved Ralph state prompts a review after restore rather than restarting work.

The protocol uses one `ClientOperation` for retries and exact native user echoes.
The greeting issues a public client id, fresh daemon instance and private
256-bit resume token. Per-client decimal counters preserve all 64 bits and never
wrap. Equal retained retries reuse their original result; changed or expired
retries never execute. A restarted daemon invalidates old retry rights and
cursors. Possibly written work needs explicit human recovery, never automatic
resend. Private results survive socket closure while their bounded lease remains.

Shared event replay, command lanes, native frames, output watches, request bytes,
reply bytes and socket writes all have explicit limits. Notable limits are
16 live owners, 16 peers, 256 saved descriptions, an encoded 1,024-frame/16-MiB
replay suffix per owner, 32 MiB of outstanding read replies plus one reply being
encoded, and 128 frames/16 MiB per socket including its active write. Slow
subscribers get a replacement snapshot or an explicit watch reset/closure;
healthy sessions and private results survive. These limits count admitted or
encoded data, not exact decoded heap. The original transcript remains complete.
See `docs/daemon-protocol.md` for all budgets and recovery rules.

## Final checks

Runtime source is `58e990100a708d322e6a113b683e42918c243846`, rebased onto main
`a29b1a1e3720e48b09e2ca984fb16c29b5857ea3`. Documentation and the repeatable
socket measurement driver follow that source without changing runtime Rust.
Main's context-size editor and contracts survive the rebase; saved host defaults
now live in core and the terminal keeps only its editor draft.

- Default workspace tests: 816 application tests passed, two ignored; 296 core
  tests passed, one ignored; nine protocol and 20 daemon tests passed; three
  compile-fail documentation tests passed.
- No-default workspace tests: 780 application tests passed, one ignored; the
  same 296 core, nine protocol, 20 daemon and three compiler checks passed.
- Strict workspace/all-target Clippy and release builds passed in both feature
  modes. The headless replay example also builds in release mode.
- `bash scripts/test.sh --release --codex-contract` passed 13 architecture
  tests, 61 compiled Claude helper tests, 16 actual daemon workflows, 183
  terminal workflows, 23 native-monitor workflows, installed Codex contracts
  and the recorder/screen/package checks. Release artifacts:
  `target/ui-artifacts/run-hzGFMz`. Debug daemon workflows also pass at
  `target/daemon-artifacts/final-debug`.
- The resolved dependency checker passes for all features/platform branches;
  core/daemon contain no terminal/audio dependencies and proto remains
  serde-only. Every changed Rust file is at most 382 lines and each function
  at most 60. `git diff --check` passes.

The process fixtures use temporary private homes and deterministic native peers,
including the compiled Claude controller. They call no model and read no user
session. They prove ordered sends, private reconnect, two identical shared
streams, independent running output, replay expiry, approval races, malformed and
oversized input, large restore, database contention, bounded slow-peer recovery,
large pending reads, EOF, repeated interrupt pressure and owned-child shutdown.
Negative tests proved that disabling key guards breaks those workflows.
The direct engineering self-review is `exec019a-engineering-review.md`, verdict
PASS WITH ISSUES for the measured fixed file-backed RSS increase. No independent
review worker ran.

Logs under `/tmp/carlos-ep019a/` include `final-control-tests.log`,
`final-control-no-default-tests.log`, both `final-control-*-clippy.log`,
`final-control-release-contract.log` and `final-debug-process.log`. Temporary
artifacts are local evidence; the methods, identities and summaries below remain
in the repository so future runs can reproduce them.

## Terminal speed and memory

The terminal comparison uses 31 rotating paired rounds pinned to CPU 6, private
configuration/state, matching default-feature release binaries, the same
1,000-turn synthetic fixture (seed 1, 24 tool lines, 160 by 48 screen), and a
captured five-row stale-transcript fixture. No build or test competed with these
measurements. Row/line/item counts match: synthetic 6,001/39,347/6,000 and
captured 5/9/4. The terminal benchmark appends its extra synthetic row.

Median milliseconds, original reference / preserved EP018a / final:

- Source apply: 14.04 / 14.08 / 14.08.
- Synthetic replay: 46.66 / 46.67 / 46.38.
- Layout: 14.73 / 14.72 / 14.53.
- Full draw: 3.42 / 3.40 / 3.32.
- Scroll p95: 0.82 / 0.82 / 0.82.
- Typing p95: 0.65 / 0.62 / 0.64.
- Working p95: 0.61 / 0.62 / 0.62.
- Append p95: 0.66 / 0.65 / 0.65.
- Captured replay: 0.97 / 0.98 / 0.91.

No paired 95% bootstrap interval for median slowdown has a positive lower bound
against either reference. For example, current-minus-EP018a replay has paired
median -0.50 ms, interval [-0.70, -0.28]; full draw -0.07 ms,
[-0.12, -0.05]. Typing's paired median change is zero with an interval including
zero. These runs establish no measured regression in these workloads, not a
universal speedup. Raw samples and summaries:
`/tmp/carlos-ep019a/final-latency/summary.json` and `final-latency.py`.

Five alternating `wait4` peak-RSS pairs at each size show EP018a / final median
KiB of 22,796 / 24,684 at 1,000 turns, 29,372 / 31,588 at 2,000, and
42,992 / 45,060 at 4,000. The difference is roughly fixed rather than growing
with transcript size. A separate five-pair `/proc` check at 4,000 turns sampled
the peak every two milliseconds: both have median anonymous RSS of 32,848 KiB;
file-backed RSS is 10,332 / 12,332 KiB. The observed increase is file-backed,
not extra anonymous transcript storage. Evidence: `final-rss/summary.json`,
`final-rss-breakdown/summary.json`, `final-rss.py`, `rss-breakdown.py` in the same
temporary directory. The CLI's old built-in RSS fields return zero on this host;
these measurements use Linux process counters instead.

## Separate headless costs

`cargo run --release -p carlosd --example perf_replay -- 1000` uses the same core
synthetic generator as `carlos perf-session`. It applies source facts, then
exports every row in 100-row pages and every original tool body in 64-KiB
chunks through a four-frame handoff, a real Unix pair and a typed decoder. It
does not retain a complete second wire log. It excludes native startup,
manager admission and terminal rendering. Writer/encoder/decoder run
concurrently, so their durations overlap and must not be added.

Thirty-one samples at each size, pinned to CPU 6, give:

- 1,000 turns: 6,000 rows, 3,698,790 source bytes; 4,882,563 wire bytes,
  1,060 frames, 1,000 output chunks. Median apply 13.98 ms, export/encode
  15.21 ms, writer wait/send 6.52 ms, client decode 3.59 ms, transfer wall
  27.55 ms. Median RSS before/apply/after transfer: 2,688/7,368/7,556 KiB.
- 2,000 turns: 12,000 rows, 7,402,040 source bytes; 9,769,058 wire bytes,
  2,120 frames, 2,000 chunks. Apply 27.98 ms, encode 30.82 ms, writer
  13.48 ms, decoder 7.01 ms, wall 55.47 ms. RSS 2,636/11,748/11,420 KiB.
- 4,000 turns: 24,000 rows, 14,809,300 source bytes; 19,542,844 wire bytes,
  4,240 frames, 4,000 chunks. Apply 56.01 ms, encode 62.12 ms, writer
  26.35 ms, decoder 13.86 ms, wall 111.36 ms. RSS 2,652/20,568/19,160 KiB.

Work grows with fixture size in these samples. RSS is a sampled process measure,
not proof of an exact worst-case heap limit. Raw evidence:
`/tmp/carlos-ep019a/final-headless/summary.json` and `headless-metrics.py`.

The separate `tests/daemon/measure.py` runs the actual daemon Unix server, owner
and real Grok adapter with an instant deterministic native peer. One thousand
send admissions measured median 0.165 ms, p95 0.228 ms and max 0.874 ms;
exactly 1,000 native prompts were observed. This includes Python client and
fixture logging costs and measures admission, not model completion. Actual
daemon RSS after greeting / after admissions was 8,128 / 15,504 KiB; anonymous
796 / 5,292 and file-backed 7,332 / 10,212 KiB. Native peer/client memory is
separate. The growing complete source remains in the owner. Reproduce with:

    target/ui-venv/bin/python tests/daemon/measure.py --binary target/release/carlosd --rounds 1000 --artifacts target/daemon-artifacts/measurement

Final raw samples and memory snapshots:
`target/daemon-artifacts/final-measure/roundtrips.json`.

## Build identities

Built with Rust 1.100.0 (`0ef1bc386`, 2026-11-09), LLVM 23.1.5,
`x86_64-unknown-linux-gnu`. Installed Codex contract peer: 0.167.1.
Metadata: `/tmp/carlos-ep019a/final-identities.json` and
`final-control-no-default-identities.json`.

- Default terminal: 20,346,176 bytes, SHA-256
  `f03799c8ef130bc5967b6eb9cb24abf3aca2e312d747a4426281ec674711f979`.
- Default daemon: 15,090,288 bytes, SHA-256
  `cd67ace01d67dfb23a72755575f0c516b931d98510ba758265133043391d642f`.
- Replay example: 1,465,936 bytes, SHA-256
  `c1984a13e291becb7b9555c922b27d4c1f530401605e6794c71ecfe42baf5a58`.
- No-default terminal: 18,212,504 bytes, SHA-256
  `81efc293b6659f8974e61f49541fc8bd6cf3ec4608b3c74357264ee99243e050`.
- No-default daemon: 15,091,160 bytes, SHA-256
  `92a9d9bf0794e7cc01fb950b06dbc635f2585f93897aac34979f57ae8a1fafbf`.
- Preserved EP018a terminal: 20,188,600 bytes, SHA-256
  `3b7970cae7f4d33cc1687715ba6990e36315482324f8e015cc72fcff04c262be`.
  Its accepted runtime source is `43b34609`, closeout `51c4a00`. The final
  terminal is 157,576 bytes (0.78%) larger.
- Original pre-EP016 terminal SHA-256:
  `c880c65cc8732b296744bca7f42be5f729ab8b78580c84f2c95113736a1c0b53`.

Preserved final executables are `/tmp/carlos-ep019a/final-carlos` and
`final-carlosd`; reference executables are `ep018a-carlos` there and
`/tmp/carlos-base/target/release/carlos`. The installed terminal remains
SHA-256 `d4595e2ac79f9ccb4cf279f08fc5dfc903e98aaec21960d7d0dc6c4727f6b2a3`.

## Next-plan gates and limits

The owner should review the daemon before authorizing EP019b. Its scope lives in
`docs/terminal-client-migration.md`. That step must add bounded original-source
reads for terminal color/copy behavior, handle plain rows/pages above eight MiB,
complete typed native task/queue/goal/settings/summary/lifecycle commands, and
prove every terminal workflow before deleting direct core access. It must also
preserve main's context-size editor through typed host-default settings.

The current daemon exposes no arbitrary native methods. Grok keeps its existing
capability limits and rejects unsupported manual approvals. Session backends keep
their native history files. A crash starts a new daemon instance and requires
explicit uncertain-command recovery; saved Ralph never resumes automatically.
The spike has no TCP listener, device pairing, language bindings or iPhone code
yet. Those remain EP020 through EP022. macOS/device checks remain future gates.
