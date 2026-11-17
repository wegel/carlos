# EP022 phone send modes and earlier-message edits

The owner asked to stop a running response, choose whether a new message steers
that response or waits for another turn, and edit an earlier user message. The
main input keeps its existing rule: an empty input shows Stop; text shows Send.
While a response runs, the input now offers a small Queue/Steer menu. Queue stays
the default, and the phone remembers the choice for each saved server.

Long pressing a user message offers Edit from here. A separate sheet reads the
complete original prompt without replacing the main draft. Opening, cancelling
and retrying that read do not start work. Fork and send explicitly creates a
branch before that prompt and sends the edit once. A running response must first
be stopped explicitly. Rejected or uncertain sends retain the edited text; the
existing recovery controls never replace a different main draft automatically.
Disconnecting while waiting releases Close rather than trapping the user in the
sheet, but does not resend the edit.

Commit12b5f879 adds `prompt/select` direction `at_entry`. A transcript row and
its immutable original prompt can have different ids, and equal text can occur
several times. The daemon therefore resolves the record through the exact row
in the chosen source and session. The shared client checks that the reply names
that row. The phone uses the canonical prompt record for text reads and rewind,
with the existing UTF-8 chunk, session and operation checks. An old daemon can
reject this new read normally; no protocol or frame guard was weakened.

Commit18f102a1 compares internal ids directly, adds an actual-listen probe to the
daemon fixture and records the terminal scroll regression. Commit2cf37413 adds
the narrow typed phone calls, explicit send mode, separate editor and actual
native app workflows. Swift does not parse backend history or resolve equal text.

## Checks

Workspace/all-target Rust checks, release Rust checks, formatting, clippy,
architecture checks and package smoke checks pass. The full release command is
in `tmp/phone-handover-tools/phone-controls-release-gate-02.log`; its package is
`target/ui-artifacts/run-2uz94A/package`, runtime source
`6d95ccf1448677ce01b8656d96b11f47`. All226 daemon cases and332/333 terminal
cases pass. The queue paging case misses one Down key and stops at34/45 rather
than35/45, then passes unchanged in1.002s under `run-2uz94A/queue-recheck`.
Keep that failed full run. Gate01 separately exposed a socket-created-before-
listen fixture race; its corrected connection probe and focused Grok recheck
pass. Neither test deadlines nor assertions were weakened.

The unchanged rest of the release script passes in
`phone-controls-release-rest-02.log`:2 selection,5 source failure,2 dictation,
1 ANSI notice, installed Codex schema/contract,23 actual Codex and10 actual
packaged Claude cases. The actual daemon duplicate-prompt test verifies the
older canonical record, the newer duplicate and retirement of the old row after
fork. The phone facade checks exact Unicode text, both send modes, read-only
editor opening, foreign row rejection and retained uncertain edited text.

Final iPhone source `803de11aabdd9e72988f` passes24 simulator units, no skips, in
`tests-20261117T181558.xcresult`. Its actual send-mode workflow passes in94.768s
(`phone-codex-afa69baba05d`, `ui-tests-20261117T182024.xcresult`): one Steer
targets the current native turn, Queue starts only after Stop, and no fork is
sent. Its edit/cancel workflow passes in89.398s (`phone-codex-d4fd31270b54`,
`ui-tests-20261117T182331.xcresult`): both cancellations and rereads preserve
the main draft, exactly one fork names the original turn, and the edited prompt
is sent once. It also checks a44pt primary action. Root inspected the actual
screens of both workflows under `target/ios-artifacts/phone-controls-final-shots`.
The rejected-fork workflow also passes in91.394s (`phone-codex-38b55c4a4aa1`,
`ui-tests-20261117T182543.xcresult`): exactly one rejected native fork leaves
the original conversation and both drafts intact, with Restore disabled while
the main draft contains text. Root inspected that captured screen too.
The final Release device build, strict signing and install all pass. The signed
bundle is `~/carlos-ios-build/previews/803de11aabdd9e72988f-20261117T232855Z/Carlos.app`,
executable SHA256 `a53fa67ce91276fc1992198709c46895cfbe5991fc0720767d49e06559432009`.
The exact iPhone18 Pro install receipt confirms `app.carlos.phone`. Launch is
denied because the phone is locked; the owner can unlock and open it. Logs are
`phone-controls-device-{build,sign,install}-01.log` under the private handover
tools directory. Intermediate source0419's incomplete run is not counted as
passing. Apple records hosting-view warnings in these
otherwise passing runs; the attachments are preserved. The existing backend
error currently includes its JSON wrapper in the rejection label. The final
EP polish pass must show its human message while retaining detailed diagnostics;
this checkpoint does not claim that error presentation is finished.

## Terminal row count and measured cost

Commit4956ca53 fixes the debug59-versus60 rendered-row assertion found during
this work. An indented ASCII call took four rows in a greedy counting shortcut,
but three in the existing painter's word-break algorithm. Long ASCII now uses
the painter's exact algorithm, like Unicode. The short ASCII fast path and
cached counts remain. Public scalar/cache checks and an actual terminal test
cover the line while scrolling through history with an untouched private draft.

Five interleaved synthetic1000-turn runs compare the frozen fa5890c2 package
with the checked6d95ccf1 package, at160x48, seed1 and24 tool lines. Both produce
39347 rendered lines. Median initial full layout rises from16.53ms to20.58ms;
the exact counting fix costs about4ms on this generated history. Median scroll
p95 is0.70ms before and0.69ms after; typing p95 is0.51ms and0.49ms. These runs
do not establish a memory comparison: the harness reports zero RSS. Raw files
are `tmp/phone-handover-tools/wrap-perf-{before,after}-[1-5].txt`, with the
summary in `wrap-perf-summary.json`.

## Installed package and armed idle daemon update

The checked CLI/daemon/helper package is installed under `~/.local/bin` as
UID1000 after verifying515 hashes and the prior files. Backups and receipt are
`~/.local/lib/carlos/backups/20261117T232124Z/install.json`; the immutable new
package is
`~/.local/lib/carlos/releases/20261117T232124Z-6d95ccf1448677ce01b8656d96b11f47`.
The active daemon remains PID1723001/sourcefa5890c2. It owns the current turn
through `carlos-phone-daemon.service`; stopping that service now would kill it.
Its crash-recovery plan is pinned to the immutable fa589 package, so installing
new executables cannot invalidate that recovery source check.

The separately prepared `phone-controls/normal-phone-handover.json` targets
only PID1723001 and its birth id. The new helper must run in its own differently
named user service. It waits for every saved session to become idle, checks the
Unix socket owner, stops that exact process through a pidfd and starts the new
daemon with the original shell environment, socket, state and private config.
It never force-kills or saves environment values in a report. A separate checker
must then prove the saved token authenticates to the new source, the current
session remains present and the Mac can reach the LAN listener. Do not claim
the new daemon is live before that post-turn proof exists.

`arm-phone-controls.py` starts `carlos-phone-controls-daemon.service` and
`carlos-phone-controls-connectivity-proof.service`. The first receipt confirms
`waiting-for-idle` targeting PID1723001; that daemon remains the main process
of its original service. The new waiter initially has PID1906940 and will
become the new daemon only after this response and any other active turns end.
The isolated proof in `phone-controls/idle-handover-proof.json` already passes
against the new6d95 package, retaining the session/device grant with zero native
prompts. Inspect the separate real `phone-controls/normal-phone-connectivity.json`
after the turn ends; it does not exist yet at this checkpoint.

EP022 remains open. The final whole-plan cost/functional checks and the owner's
single end-only review still remain; this checkpoint does not approve or archive
the EP.
