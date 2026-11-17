# EP022: resume this conversation inside Carlos

The owner asked Root to stop after fixing Carlos's large Codex history read so
the owner can resume this conversation in Carlos itself. Continue only EP022
after that handover, directly, without subagents or intermediate reviewers.
Worktree: `/var/home/wegel/work/perso/carlos-daemon`, branch `iphone-spike`.
The separate main checkout remains at a29b1a1e3720e48b09e2ca984fb16c29b5857ea3.

## Repair

Commit28c778e changes native Codex history reads from100 complete turns to one.
The owner session yields83618724bytes of complete native history, including one
47477424byte turn. The100-turn reply exceeded the existing64MiB stdout cap.
Carlos closing that pipe then caused Codex's Broken pipe message. The small
resume reply already excludes turns and was not the cause.

Carlos now returns cached older rows before fetching another turn. Two clients
can share the same newly fetched prefix even when it contains fewer than their
requested maximum100 rows. The existing8MiB event cap,64MiB history cap, original
read budget and20s deadline stay unchanged. A future single turn above64MiB would
still need item paging; this repair does not remove that safety limit.

Runtime source is284b3acc7aa71615fed2cef9509c316b. Commitff3bb8b separates the
native test traversal helper without changing its checks. Debug and release Rust
checks pass. Focused checks pass15 daemon tests,20 terminal history tests and
both actual Codex pipe/socket ancestor tests. Each native test reads31 turns,
1711 items and75737895bytes without inference, losing the ancestor cut or making
per-item calls. A fresh434141262byte isolated copy of the owner's actual session
reaches a ready terminal in3.225s, with one889761byte reply/311 items and no prompt.
The original owner session was not changed by these probes.

The full release/backend check is recorded in
`/tmp/carlos-ep022/codex-history-release-gate-01.log`. Its package lives under
`target/ui-artifacts/run-iPphff/package`. The staged installer and rollback copies
are in ignored `tmp/install-carlos-284b3acc/`. Root installed this matching package
and verified both installed source markers. The receipt/rollback copies are in
`~/.local/lib/carlos/backups/20261117T203941Z`. Root's UID0 maps to hostUID1000;
the installed files still belong to the normal user.

The original shell gate passed Rust/clippy,225 daemon checks,330 terminal checks,
package/source/copy checks, native Codex contracts and22/23 native UI cases. One
test needed Home to fetch an older child result; its corrected case passes1/1.
Packaged Claude passed9/10. Its context test sampled an input-only native reply
before the completed tool step; waiting for that step keeps the original
40128..45000 assertion and10s deadline, and the corrected case passes1/1.
No executable changed during these test corrections. Do not claim the original
shell run exited0. Original failures and all follow-up logs remain in
`/tmp/carlos-ep022/codex-history-*`. The helper binary and relinked controller
hashes match the previous install exactly. No Claude runtime code changed.

A quiescent synthetic perf-session check covers1000 turns/6001 rows: scroll
p95 1.70ms, typing p95 1.44ms, append p95 0.57ms. RSS is unavailable/zero in this
environment. This is a diagnostic run, not the final EP's matched cost comparison.

## Normal-shell handover

The helper is `tmp/codex-large-resume/resume-fixed.py`. Run it from the owner's
normal user shell after the matching package is installed. It verifies both
installed source markers, identifies the normal Unix socket's actual process,
refuses busy sessions, stops only that daemon and starts Carlos to resume this
conversation. It does not touch the phone-preview socket or port33731.

Root cannot signal the normal daemon from its PID namespace: SO_PEERCRED reports
PID0 for that outside process. Never signal PID0. Do not start its replacement
from the tool container. The normal socket is `/run/user/1000/carlos/daemon.sock`;
its catalogue is `~/.local/state/carlos/daemon`. The normal daemon currently has
no phone WebSocket listener. After the owner resumes, finish its phone route and
device token using the same catalogue and a separate port, likely33732.

## Remaining phone work

The phone still has accepted native-list sourcede74af3e4ef7529f93d6. The owner said
its scrolling is awesome. Keep that behavior and the normal iOS flick speed.
Do not stop the owner-run preview daemon on33731.

Saved-server source23952a4b8d31245275a0 now passes actual hosts edit/remove and
duplicate-name UI/native checks. Artifacts:phone-codex-6be4fac18ff8 and
phone-codex-7909d4df9067, with original selectors/failures retained. Credential,
relaunch and token-update checks also pass on23952, with artifacts
phone-codex-615472c540ea, phone-codex-8157725b7749 and phone-codex-45e4ab9e8d99.
No newer app has been signed or installed. No tests/builds/fixtures remain running.

Large-source memory work, the final functional/cost sweep, matched Linux timings,
production counts, Root's final engineering review and ONE end-only Fable5.2
medium whole-roadmap review remain. Do not approve/archive EP022 yet or start
another EP. The previous restart handoff and private archive remain useful.
