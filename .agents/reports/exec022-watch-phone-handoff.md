# EP022 watch replies and normal phone access

The owner saw `output watch was superseded` and repeated `F5 reconnects: body
reply has a different entry, revision, cursor or byte interval` in two terminal
sessions. A cancelled watch still owned its pending RPC request in the shared
client. Its late reply could therefore show an obsolete error or try to apply an
earlier cursor after the reader had fetched newer bytes.

Commit5dc0abfc retires pending watches by session, entry and body format when a
reader closes, pauses or replaces them. It keeps the current request when the
same token starts another watch. The terminal also clears its pending watch id
when it pauses that watch to pull a large live backlog. The existing checks for
body identity, revision, cursor, interval, frame size and cache size stay strict.
Current-watch failures remain visible. No prompt is resent.

Two actual-terminal regressions fail against frozen package195960d6 and pass
against the corrected debug pair. They delay an old error past a replacement,
and delay an old valid byte chunk until a200KiB live backlog has drained. They
assert the new tail, editable private draft, no new greeting and no session send.
Four client tests cover both body formats, pause, token reuse, old chunks and
current failures. All209 client checks and the workspace/all-target debug checks
pass. The checked runtime source isfa5890c2b5fc4cf106fd45f1217b84fe.

Commitfb414108 also follows the owner's requested build policy: compatible
protocols can share the managed daemon by default. CARLOS_STRICT_DAEMON_BUILD=1
restores exact source matching. Protocol checks and an explicitly requested
native Codex endpoint remain mandatory. All15 launcher workflows pass in the
release run, including saved listener settings and authentication after restart.

The full release/native command is recorded in
tmp/phone-handover-tools/watch-compatible-release-gate-02.log. It passes Rust,
clippy, helper checks, launcher checks and224/225 daemon cases. The Claude
settings disconnect case times out waiting for a private outcome, then passes
unchanged in isolation in watch-daemon-recheck-01.log. Preserve the failed run;
do not claim its shell exited successfully. The remainder runs unchanged from
the packaging step in watch-release-remainder-01.log against the same source.
The terminal suite passes331/332; the Claude UUID-label case misses its temporary
queue notice, then passes unchanged in2.250s in watch-ui-recheck-01.log. Preserve
that failed run too. The remaining selection, source failures, dictation, ANSI,
installed Codex contract, all23 native Codex cases and all10 packaged native
Claude cases pass in watch-release-native-01.log. No runtime changed between
these checks. Neither assertion counts nor deadlines were weakened.

The new iPhone source122768f838c06ad4231a includes the shared-client fix and
keeps the accepted scrolling and saved-server screens. All24 simulator tests
pass in tests-20261117T171003.xcresult on the owned Mac. Strict signing passes.
The immutable signed bundle is
~/carlos-ios-build/previews/122768f838c06ad4231a-20261117T221212Z/Carlos.app;
its executable SHA256 is
d5d462ad479ed5a698dbcfa42f8841528bb523e3f6b83b0d2715d0b9ae3b0bc9.
The first device install loses its XPC connection. Retrying the same bundle
installs and launches it on the exact iPhone18 Pro. Keep both install logs.

The normal daemon still runs source284b3acc in PID1458745 and hosts this turn.
The checked fa5890c2 package is now installed under~/.local/bin as UID1000.
All515 package hashes and saved old executables verify. The receipt and rollback
files are ~/.local/lib/carlos/backups/20261117T220849Z/install.json; the immutable
package is ~/.local/lib/carlos/releases/20261117T220849Z-fa5890c2b5fc4cf106fd45f1217b84fe.
Both installed executables report the checked source. The staged package is
tmp/install-carlos-fa5890c2. The real running daemon is deliberately unchanged
until this active turn and the other session's active work finish.

The selected phone endpoint is ws://192.168.3.11:33732. The normal catalogue has
its own iPhone18 Pro device grant; the token stays in the private mode0600 file
~/.config/carlos/iphone18-pro.token. The saved mode0600 daemon.json selects that
LAN listener. Keep token contents out of repository reports and review context.
The phone needs a new saved server at that address with its private-network
toggle enabled and the normal token. Old preview tokens belong to other catalogues.

The checked idle handover keeps the original daemon alive during a synthetic
active turn, stops it only after all sessions are idle, and preserves the same
session and device grant. Its final proof uses sourcefa5890c2 with zero native
prompts. After the final install, arm-normal-phone.py successfully starts the
normal carlos-phone-daemon and carlos-phone-connectivity-proof user services.
The first receipt says waiting-for-idle and identifies the original PID1458745.
The service waits for
all sessions to be idle and preserves the original normal-shell environment;
the second service checks authenticated access and LAN reachability after the
restart actually begins. Do not claim the real listener is ready before that
check completes. Existing terminal processes need to be relaunched for this
client fix; F5 alone leaves their old executable running.

The final package's synthetic1000-turn/6001-row perf-session diagnostic records
scroll p95 .69ms, typing .49ms and append .54ms. RSS reports zero, so it supplies
no memory evidence. This is a diagnostic run rather than a paired speed claim.
Its transcript is tmp/phone-handover-tools/watch-client-perf-01.txt.

EP022 remains open. The separate passive debug scroll observer04 hit the existing
render-cache count assertion59 vs60 at render_cache_state.rs350. Its artifact is
not a clean runtime check; investigate that discrepancy after this urgent
handover. The final EP functional/cost matrix, the scoped large-source prefetch
work, direct engineering review and one end-only Fable review remain pending.
No new reviewer or developer agent ran. Main and frozen previews remain unchanged.
