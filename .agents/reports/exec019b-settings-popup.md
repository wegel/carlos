# Edit daemon settings in the terminal

The opt-in socket terminal now opens Ctrl+M using checked model pages and the shared settings editor/painter. Each terminal keeps its own unsent model, effort, summary, context, label and appearance choices. Core owns the accepted native choices and host defaults. The terminal neither creates SessionState nor sends arbitrary native methods.

The editor requests at most32 models per page and retains one pending page until the matching numbered facts arrive. It checks the page offset and exact settings revision, restarts an obsolete read and bounds model text to eight MiB. A refresh keeps edited fields while untouched fields follow new facts. The old shared rules preserve unknown summary values and hide effort for a model that does not offer it. Saving an untouched placeholder does not pin an invented default.

Save validates every field before sending work. An edited context choice writes the daemon host's next-launch file first. Native model choices retain the original catalog revision. A separate label-only command can follow newer model facts only while the original label boolean still matches. It cannot rebase or resend the native model intent. The three commands keep private operation identities and settle separately; this is not an atomic transaction. A definite context failure stops other writes. A definite native refusal can still save the independent AI-label and device appearance choices, as the old terminal did. An uncertain result stops the remaining steps and keeps the original operation until an explicit refresh.

The accepted appearance choice updates only this terminal's themes and tool-output mode. One bounded off-thread job writes those fields through the shared locked writer. It never writes the host AI-label flag. A held file lock leaves typing usable; a write failure reports the accepted live choice and the file error without retrying native work.

Closing a pending popup keeps its original private result. A later refusal remains visible in that terminal's status line and editor while its composer stays unchanged. A same-daemon reconnect recovers the result without resending. A replacement daemon keeps edited fields, requires fresh facts and sends no new mutation until explicit Save. F6 reads facts; it does not retry an uncertain write.

## Exact checked source

Commit `09f411d` contains the isolated source tree `a4b8c34154511c6d278babed85fa53edc946f746`, base `3896b21` plus20 settings-only paths. The checkout is `/tmp/carlos-ep019b/settings-popup-index-check`; `/tmp/carlos-ep019b/settings-popup-index.patch` records its exact patch. Unfinished question, status and startup-worker changes were excluded from this source. Changed Rust files have at most374 lines and functions at most58 lines.

The complete default check `bash scripts/test.sh --codex-contract` passes:1465 Rust cases with10 ignored,93 daemon workflows,265 terminal workflows, two selection, five source-failure, two local-dictation and23 native-monitor cases. The runner also passes the architecture/dependency checks,70 compiled helper tests, package smoke and installed Codex contracts. The 265 terminal cases include 16new actual socket-popup workflows; the runner still exercises old embedded workflows too, so this is not proof of full terminal cutover.

The 16popup cases check two private editors, unknown summaries, original revision rejection and explicit refresh, held saves and native controls, late refusal after closing, same-daemon/private recovery, replacement-daemon no-resend, no-effort models, native Claude setters, Grok future-prompt choices, independent labels/themes, a held device file, malformed context repair, daemon-shell policy, attached Codex and unsupported Grok.

Strict workspace/all-target Clippy passes. Logs are `/tmp/carlos-ep019b/settings-popup-index-gate.log` and `settings-popup-index-clippy.log`. Runner artifacts are `run-AZ3nyL` under the isolated target's ui-artifacts directory. No-default workspace tests and strict lint also pass in settings-popup-index-no-default-tests.log and settings-popup-index-no-default-clippy.log. The normal integrated commit hook passes cargo fmt and the full workspace test suite in settings-popup-commit.log.

The frozen executable identities are in `/tmp/carlos-ep019b/settings-popup-final-frozen/identity.json`. This is a dev-profile check, not a release claim:

- Terminal SHA256 `cec30074d4aa46b1ed645b70f6699118834e61cc762b60ba5fbfb2359f70b9ea`.
- Daemon SHA256 `295d508aa6da579f3e2e14ce96fc4f4a7b0dc4122c7252ca16c40526dc33d2af`.
- Compiled controller SHA256 `4e571e98bb57b1f3307ed394e8a8a9ce62c096afe9521b8343d9f5f2dd9037d7`.

## Discoveries retained with the evidence

The first combined run caught a real neighboring-route regression: the context helper extraction had dropped ToolLabelsSet from the manager, causing a panic. It also lacked ContextSet's ordinary ingress class. Commit 14d1965 restores both and adds dispatch/reserve regressions; eight label and seven context daemon cases pass together afterward.

A new late-refusal test fails with the older popup binary because the error stayed hidden after close. The final source also copies that private failure into the terminal status hint. The fixture that reopens a failed editor now waits for its visible Thinking row, since the same error may already be visible outside the popup. A context-refresh case waits for the fresh context response before Save; early Enter during refresh intentionally sends nothing. Grok's arrow test now names the actual advertised selected effort rather than assuming a fixed index. Rejected logs remain under `/tmp/carlos-ep019b/settings-popup-combined-01.log` through `combined-04.log`; the frozen 16-case repeat passes in `combined-05.log`.

Forms, caller metadata, broader startup/discovery and status parity continue separately. The embedded path remains until its explicit deletion gate. No reviewer, EP approval, install, main change, merge or push ran for this checkpoint.


## Cancel after a local validation error

Commit `a955c7c` follows the initial popup with one old-workflow correction. Escape after an invalid unsent 99k choice now discards that edit when the popup reopens. Native refusals, uncertain writes, pending operations and disconnect recovery still keep their original drafts. The new actual terminal case fails against the preceding frozen popup and all 17 popup cases pass after the fix. Logs are settings-popup-cancel-before.log and settings-popup-cancel-after.log.

The same commit registers the existing seven context cases in the regular daemon runner. It uses the same mixin pattern as the other families, so standalone and combined checks each run every case once. An intermediate runner imported a TestCase directly and ran those seven twice; the final runner removes that duplicate.

The final isolated tree is `9ab3f08e6683b437c38f228f5d9ba55f47c0873e`, with a complete passing default/Codex gate in settings-cancel-index-gate.log:1467 Rust cases plus 10 ignored,100 daemon,266 terminal and the same selection/source/dictation/native remainder. The normal integrated hook passes in settings-cancel-commit.log. Exact dev-profile executable identities are in /tmp/carlos-ep019b/settings-cancel-final-frozen/identity.json. Whole-EP release checks remain open.
