# Terminal cutover draft

The draft lives at `/tmp/carlos-ep019b/terminal-cutover-prep`; its Cargo output lives at `/tmp/carlos-ep019b/cutover-target`. The branch still runs the embedded default until the separate deletion commit joins the migrated tests. None of these checks approve the EP.

The default route now parses options, handles help/offline perf, uses an explicit daemon socket when supplied, and otherwise calls the existing managed launcher. The terminal retains its private composer, widgets, render cache and dictation. It drops AppState, backend setup, native report consumers, the merged native/UI loop and native settings/panel wrappers. The private deletion manifest lists87 runtime files with their previous hashes at `/tmp/carlos-ep019b/cutover-runtime-delete-manifest.json`. Other obsolete test-only bridge files await their exact successor map before the branch deletion.

Both default and no-default-feature production strict Clippy builds passed before the physical deletion. The build after removing the first72 files also passed strict no-default Clippy. The draft keeps pure immutable Core models and exported offline perf rows; it does not keep a frontend session owner or backend client.

The frozen pair at `/tmp/carlos-ep019b/cutover-default-pair` has matching source marker `60be796480143a4ea0347638ba1f5f4a`. Its identity.json records the two executable hashes and packaged helper hash. It was built from the private cutover draft before the subsequent test moves and physical removals, so it proves that draft runtime, not the final branch tree.

All9 managed launcher cases pass in8.048s with the fixture default omitting `--daemon`. These check concurrent starts, one native resume, private composers, detach/reattach, startup cwd, missing/incompatible companion, live-owner mismatch, explicit sockets and aliases, and the new-child-only five-second timeout. Log: `/tmp/carlos-ep019b/cutover-default-launch.log`.

Three footer cases pass on actual Devices with greeted clients and accepted facts. They retain the original quiet/mode/deadline, narrow context/quota/right-edge and draft/error/pending assertions for both backends. Two history cases preserve an opaque edit target and scroll to the selected prompt at100x6 through the actual Device painter. Native fork/truncate assertions remain in Core successors; the client never truncates shared facts while editing. Logs: `cutover-status-footer-02.log` and `cutover-history-view-02.log` under `/tmp/carlos-ep019b`.

The existing phone help workflow now checks exact one-row Up, five-row PageUp and complete Home frames for Codex and Claude, retaining zero-native-chat assertions. It passes in5.274s against the frozen pair; log `cutover-exact-help.log`. The full298-workflow run is still active in `cutover-default-full-ui.log`; no complete-gate pass is claimed yet.


## Refreshed preflight after native owner moves

The separate /tmp/carlos-ep019b/deletion-join-preview starts at8e07c18. It keeps every current pure frontend test, including the now-pure tool-output and weekly indicator suites, plus the local-input/perf hooks. Native goal8/panel11 and ten real pipe schedules remain registered in Core. All-target no-default workspace checking passes using its own fresh deletion-join-target.

The preview omits ten still-pending native frontend modules explicitly in cutover-preview.json. This excludes it from final acceptance. The original87-path runtime manifest now removes83 additional files; four paths already retired on the branch. All current remote modules stay untouched by the removal patch. Five merge choices are recorded in the JSON: daemon-only CLI, pure composer, daemon-default runner, pure notification helpers and the current borrowed picker painter.

The old pure dictation test now keeps its exact composer/chord assertions; its private rewind assertion belongs to the existing actual Device test matching_final_cancels_old_prompt_reads_without_changing_an_edit_target, which preserves the exact before-rewind text across final dictation and late prompt replies. The final default-feature gate must run that test. No new view-history owner is added to satisfy the removed field.

This preflight changes no primary runtime file. C2 still owns the checked Esc/private-goal proofs, the backend worker still ports its remaining native families, and the final removal commit plus release/runtime/performance gates remain open.

The refreshed preview also passes all344 retained frontend tests with seven ignored under no-default features, using deletion-join-target. This remains a pure/frontend preflight; it is not the final complete workspace/runtime gate.
