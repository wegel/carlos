# Terminal cutover draft

The draft lives at `/tmp/carlos-ep019b/terminal-cutover-prep`; its Cargo output lives at `/tmp/carlos-ep019b/cutover-target`. The branch still runs the embedded default until the separate deletion commit joins the migrated tests. None of these checks approve the EP.

The default route now parses options, handles help/offline perf, uses an explicit daemon socket when supplied, and otherwise calls the existing managed launcher. The terminal retains its private composer, widgets, render cache and dictation. It drops AppState, backend setup, native report consumers, the merged native/UI loop and native settings/panel wrappers. The private deletion manifest lists87 runtime files with their previous hashes at `/tmp/carlos-ep019b/cutover-runtime-delete-manifest.json`. Other obsolete test-only bridge files await their exact successor map before the branch deletion.

Both default and no-default-feature production strict Clippy builds passed before the physical deletion. The build after removing the first72 files also passed strict no-default Clippy. The draft keeps pure immutable Core models and exported offline perf rows; it does not keep a frontend session owner or backend client.

The frozen pair at `/tmp/carlos-ep019b/cutover-default-pair` has matching source marker `60be796480143a4ea0347638ba1f5f4a`. Its identity.json records the two executable hashes and packaged helper hash. It was built from the private cutover draft before the subsequent test moves and physical removals, so it proves that draft runtime, not the final branch tree.

All9 managed launcher cases pass in8.048s with the fixture default omitting `--daemon`. These check concurrent starts, one native resume, private composers, detach/reattach, startup cwd, missing/incompatible companion, live-owner mismatch, explicit sockets and aliases, and the new-child-only five-second timeout. Log: `/tmp/carlos-ep019b/cutover-default-launch.log`.

Three footer cases pass on actual Devices with greeted clients and accepted facts. They retain the original quiet/mode/deadline, narrow context/quota/right-edge and draft/error/pending assertions for both backends. Two history cases preserve an opaque edit target and scroll to the selected prompt at100x6 through the actual Device painter. Native fork/truncate assertions remain in Core successors; the client never truncates shared facts while editing. Logs: `cutover-status-footer-02.log` and `cutover-history-view-02.log` under `/tmp/carlos-ep019b`.

The existing phone help workflow now checks exact one-row Up, five-row PageUp and complete Home frames for Codex and Claude, retaining zero-native-chat assertions. It passes in5.274s against the frozen pair; log `cutover-exact-help.log`. The full298-workflow run is still active in `cutover-default-full-ui.log`; no complete-gate pass is claimed yet.
