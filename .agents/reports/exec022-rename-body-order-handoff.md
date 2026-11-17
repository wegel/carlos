# Phone names, Steer default and body reply order

Commit `47ec832b` adds Session settings → Rename session. A separate private name
draft leaves the message composer untouched. Cancel sends nothing; Save uses
the existing typed session/rename command once. Accepted daemon facts update
the header and session list. An uncertain result keeps the name and disables
immediate retry. The user can close, check the actual name and explicitly open
another editor. Rename errors stay in that editor rather than leaving an old
error banner after a later successful rename.

The owner's later request supersedes the previous Queue default. Every new
message starts with Steer. Queue remains an explicit choice for that draft.
Clearing or sending that draft restores Steer, and old saved Queue preferences
are ignored. Stop still replaces Send only when the main input is empty.

## Remaining F5 error

The private zmx capture `tmp/phone-handover-tools/reconnect-07-carlos.txt`
records `F5 reconnects: body reply has a different entry, revision, cursor or
byte interval` in the live terminal footer. A Tool output loading-source line
appears immediately above it. That is a transcript row, not a separate viewer.
Both open terminal processes and the live daemon use `fa5890c2`, which already
contains the cancelled-watch repair. This is not dismissed as an old build.
Read-only observers 06/09 scroll the live session without reproducing it;
observer 08 separately sees command-queue pressure. No observer sends a prompt.

Commit `c6b9fc02` fixes another concrete race: a completed source/output read can
outrun the numbered facts identifying its new body version. The manager now
holds that reply until the peer's session stream reaches its sequence, retaining
the existing byte reservation and asking the owner for current facts. The rule
covers direct reads and watch chunks. Readers without a session stream still
receive direct replies immediately. No cursor, revision or byte check in the
client was weakened, and no frame, queue or time limit changed.

The deterministic manager test fails before the fix because the reply escapes
before its version facts. All four source/output/direct/watch cases pass after
the fix and check the actual wire order and retained byte charge. Another test
checks the caller without a session stream. The actual terminal repeatedly
replaces a tool's output, checks every body sequence against earlier facts and
keeps its private draft with one greeting and no send. That terminal workflow
passes on both pairs; the unit test forces the rare ordering race. The capture
does not establish that this race explains every past reconnect report.

## Checks and phone build

All 34 phone Rust checks pass. The commit hooks also run the complete debug Rust
tests. Final phone source `6cb590365ff8b4e6aa02` passes 25 native simulator checks,
no skips (`tests-20261117T184945.xcresult`). Its rename workflow passes in 65.254s
(`phone-codex-3ee66c4f109d`, `ui-tests-20261117T185028.xcresult`), preserving
the name and prompt across cancellation, an uncertain result and a new explicit
save. Its Steer-default workflow passes in 88.050s (`phone-codex-d99c9511c76d`,
`ui-tests-20261117T185235.xcresult`), proving one actual native Steer and a Queue
that starts only after Stop. Root inspected the captured rename screen.

The first rename workflow incorrectly expected a backend error to mean definite
rejection. The existing engine reports it conservatively as uncertain, and the
app correctly disables editing/retry. Preserve `phone-rename-ui-01.log`; the
corrected test checks that state and makes the next save only after checking
the actual session. The earlier debug gate also retains the known Claude disk
outcome timeout. That same case passes in the final release run.

The release package source is `e77e5a338758f2193504033f0d68e278`, frozen under
`target/ui-artifacts/run-q4G1m7/package`. Rust, clippy, architecture, helper,
launcher and all 226 daemon checks pass. The remaining terminal/native checks
are still running in `tmp/phone-handover-tools/body-order-release-gate-01.log`.
The separate checked installer is staged at `tmp/install-carlos-e77e5a33` and
verifies 515 package hashes plus the prior installed files before any replacement.
The Release phone build, strict signature check, exact iPhone18 Pro install and
launch all pass. The immutable app is
`~/carlos-ios-build/previews/6cb590365ff8b4e6aa02-20261117T235553Z/Carlos.app`,
executable SHA256 `fa1934f388a31123f22bafac8aad525c8f4cd8905b6d384749a1bca16bce70fc`.
See `phone-rename-device-{build,sign,install}-01.log` for receipts. The new Linux
package has not yet been installed; its final terminal/native checks still run.

## Safe activation

The live daemon is still PID 1723001, source `fa5890c2`, and owns this turn through
`carlos-phone-daemon.service`. Do not stop it during an active turn. The earlier
`carlos-phone-controls-daemon.service` was stopped only after checking that its
main process was the Python idle waiter, not a daemon. This prevents installing
the now superseded `6d95` package when the sessions next become idle. The current
daemon's own crash recovery remains pinned to its immutable fa589 package.

After the final checks, install the new checked CLI/daemon/helper package, point
the separate handover plan at its immutable package, re-arm the idle waiter and
checker, then end the turn. The helper must wait for every saved session to be
idle and preserve the normal shell environment, state, socket, listener config
and saved phone grant. Only a later authenticated connectivity receipt can prove
the new daemon is live. EP022 remains open; this report is not its final approval.
