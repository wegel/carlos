# Refresh compatible Rust crates and report larger upgrades

## Scope and working rules

The user chose compatible updates and a report of larger upgrades. Keep the
current Cargo.toml constraints, including exact tui-markdown 0.3.13, and refresh
Cargo.lock without yanked versions. Preserve the reviewed Markdown fix, Claude
migration, installed regular executable pair, and durable rollback backup. One
implementation agent works sequentially; parent reviews before any install.
Do not commit, publish, spawn agents, or call a remote model.

## Progress

- [x] Parent audited direct dependencies and reviewed the focused upstream table fix: PASS.
- [x] Save the pre-refresh manifest/lock under `/tmp/carlos-compatible-refresh-ueg61wti/`.
- [x] Refresh compatible direct/transitive versions and record exact changes; Cargo.toml constraints are unchanged.
- [x] Read material release changes and keep eight larger direct migrations as reported-only work.
- [x] Supply exact-version Markdown/objc2 notices; strict notice preflight passes all four shipped target graphs.
- [x] Run combined release/contracts, strict lint/style, and no-default-feature checks.
- [x] Compare alternating old/new release runs on ordinary and table-heavy fixtures; parent independently confirmed no material regression.
- [x] Parent final review returned PASS and explicitly authorized the local regular-file install.
- [x] Preserve the SDK baseline backup, install reviewed Carlos, and pass focused installed table/native smoke without replacing the helper.

## Surprises & Discoveries

Parent's audit lives in `/tmp/carlos-crate-audit-qavrxrw6/`. The initial dry run
predated the move from an old Markdown fork to upstream 0.3.13, so its 129-package
forecast will differ from the final graph. Ratatui 0.29 requires unicode-width
0.2.0 exactly for its own text; do not force a newer incompatible width crate.
The helper's SDK/Bun graph remains pinned and outside this Cargo refresh.
Cargo resolved 126 package operations. The lock now has 297 packages (previously
288), with 140 crate names changing their version sets, being added, or being
removed. All 296 registry entries were checked against Cargo's current sparse
index cache: none were missing and none were yanked. The remaining package is
Carlos itself. Ten direct crates changed in this refresh; pulldown-cmark had
already reached 0.13.6 in the upstream table task, so all eleven eligible direct
crates from the original audit now match their compatible current releases.

Newer base64 0.23.1 and shlex 2.0.1 appear transitively through updated plist and
cc. Carlos's direct base64 0.22.1 and shlex 1.3.0 remain unchanged. Tokio enters
only through the wasm-bindgen-futures/cpal graph for WebAssembly, not the native
Linux runtime. The refreshed graph's highest declared Rust requirement is 1.89
(rusqlite/uuid); the actual compiler used here is 1.98.1.

## Decision Log

- Use Cargo's resolver against the current manifest; do not raise version floors merely to rewrite constraints.
- Retain the exact upstream Markdown pin and reproducible lockfile.
- Report eight larger upgrades without changing their incompatible current ranges: base64, cpal, ratatui, ratatui-interact, ratatui-textarea, rubato, shlex, and unicode-width.
- Review text wrapping/Unicode, serialization/configuration/UUID, core rendering, and native dependency changes with their primary release notes and code.
- Keep notice checks strict; add pinned source evidence if a new crate archive omits its license.
- Run the final combined release suite once the refreshed graph and notices are ready. Use only deterministic peers or native CLI plus localhost fake endpoints.

## Version report and behavior review

Direct changes in this stage:

- anyhow 1.0.102 to 1.0.104; libc 0.2.182 to 0.2.191.
- ratatui-core 0.1.0 to 0.1.2; Carlos's main ratatui 0.29.0 stays fixed by its current range.
- rusqlite 0.41.0 to 0.41.1; serde 1.0.228 to 1.0.229; serde_json 1.0.149 to 1.0.154.
- textwrap 0.16.2 to 0.16.4; unicode-segmentation 1.12.0 to 1.13.3.
- toml 1.1.2 to 1.2.1 (both declare TOML spec 1.1.0); uuid 1.23.1 to 1.29.2.

Textwrap now uses ICU for Unicode line-break opportunities instead of the older
unicode-linebreak crate. Its optional complex-script feature remains disabled;
the current default uses the non-complex-script segmenter. This adds ICU data
and makes count/materialized-layout and performance checks especially relevant.
[Upstream changelog](https://github.com/mgeisler/textwrap/blob/master/CHANGELOG.md).

Serde's patch updates its derive macros to syn 3. Serde JSON fixes invalid UTF-8
error positions and several optional precision/map modes. Carlos keeps its
existing feature choices and JSON wire formats; protocol and saved-state tests
remain the checks for behavior.
[Serde release](https://github.com/serde-rs/serde/releases/tag/v1.0.229),
[JSON releases](https://github.com/serde-rs/json/releases).

TOML fixes duplicate-key checks/key writing, preserves datetime values, and
reduces allocations; 1.2 adds serializer improvements. Existing dictation
configuration tests check the affected parser path.
[Upstream changelog](https://github.com/toml-rs/toml/blob/main/crates/toml/CHANGELOG.md).
UUID's newer notes include version-7 ordering and WebAssembly changes; Carlos
continues to request only version-4 UUIDs.
[UUID releases](https://github.com/uuid-rs/uuid/releases).

Larger direct upgrades were checked but not applied:

- Terminal UI group: ratatui 0.29.0 to 0.30.2, ratatui-interact 0.2.0 to 0.5.4, ratatui-textarea 0.8.0 to 0.9.4, and Carlos's unicode-width 0.1.14 to 0.2.3. Review these together because they share widget/input/display-width types. Ratatui 0.29 separately requires unicode-width 0.2.0 exactly.
- Audio group: cpal 0.17.3 to 0.18.2 and rubato 2.0.0 to 5.0.1. A later task must review stream callbacks, device errors, resampler buffers, and real capture behavior.
- Encoding/command parsing: base64 0.22.1 to 0.23.1 and shlex 1.3.0 to 2.0.1. Their direct version-range upgrades remain deferred; Carlos still calls 0.22.1 and 1.3.0 even though newer copies are required transitively elsewhere. This audit does not establish whether those direct upgrades would require source changes.

The machine-readable report includes the current constraints, direct before/after
versions, eight reported-only versions, repositories, and complete lock changes
under `/tmp/carlos-compatible-refresh-ueg61wti/`:
`direct-and-larger-report.json`, `lock-changes.json`, and
`registry-yanked-check.json`. Parent's independent manifest/lock audit also
passed: no floor or incompatible direct upgrade changed.

## Notices

Upstream tui-markdown 0.3.13 omits its workspace licenses from the crate archive;
the exact release commit's MIT/Apache files now accompany the package. macOS
notice checks also exposed nine objc2-family archives that omit root notices.
Their exact `.cargo_vcs_info.json` commits now supply upstream LICENSE.md, plus
the same pinned standard MIT text used for declarations by reference. URLs and
SHA-256 values accompany every supplemental file. No notice check was weakened.
RealFFT 3.5.1 supplies its own license, so the old 3.5.0 supplement is not copied
into the new package. Historical supplements remain available for older locks.

Strict target-specific notice preflight passed: Linux 214 Rust entries, Intel
macOS 221, ARM macOS 220, Windows 218. These checks inspect resolved input notices;
they do not claim foreign runtime tests were executed. Evidence:
`notice-preflight.json` and target folders under the same refresh artifact root.
Parent independently verified all 35 supplementary file checksums and matched
the objc2 workspace commits and exact Markdown release license bytes.

## Validation and outcomes

The final command passed:
`rtk proxy bash scripts/test.sh --release --codex-contract --claude-contract`.
Artifacts: `target/ui-artifacts/run-jjwjBc`.

- Optimized Rust: 483 passed, one ignored.
- Helper: 51 source and 51 standalone tests passed, 272 assertions each.
- Package hostile-cwd, explicit environment, session and validator smoke passed.
- Terminal: all 117 workflows passed, including streaming table reflow and the narrow fallback.
- Installed Codex 0.167.0 contracts and all three native monitor cases passed.
- All four native Claude 2.1.322 localhost UI cases passed.
- Default/no-default strict Clippy, no-default check and optimized build, rustfmt, present-file style, and whitespace passed.

The runner restored the default-feature release executable after the isolated
no-default build. Its SHA-256 is
`d00a4c1c31da9e4a35a1d9517671b1ec0ec3a080ae33749c640385af6ebe2ba1`;
10,782,544 bytes, 83,504 bytes larger than the installed SDK baseline. At that
review handoff, no installed file had changed. The helper remains byte-identical to the regular installed
helper, SHA-256
`278fdcc4539b1973d00b62ad2075af079dc18bfa2ea0d0dbc2a961938ccc2744`.

The installed SDK baseline is preserved at
`/tmp/carlos-parent-table-baseline-8yd3mdop/carlos-before-table-wrap` and
`/tmp/carlos-before-markdown-table-reflow`. Compare the same 1,000-turn synthetic
input at 160x48 with 24 tool lines and seed 1, plus the parent's 200-message table
fixture at 120/54/36 columns, using alternating sequential release runs.

Five alternating sequential runs passed. Agent median before/after milliseconds:
synthetic replay 47.44/48.72, layout 14.94/15.17, scroll p95 0.86/0.81, typing p95
0.59/0.61. Both render 39,347 lines. Parent independently measured replay
48.24/47.88, layout 14.87/14.91, scroll 0.82/0.91, typing 0.63/0.69. The ordinary
UI differences remain within normal run variation; input/scroll draws stay below
one millisecond. No material ordinary UI regression appeared.

For 200 table messages, agent before/after layout milliseconds at 120/54/36
columns were 7.14/4.65, 7.22/6.69, and 7.18/7.86. Parent independently measured
7.12/4.64, 7.16/6.15, and 7.49/8.08. Correct per-cell padding/wrapping changes row
heights and removes detached wrapped border fragments: total line counts change
3,003 to 2,203; 4,807 to 4,007; and 7,181 to 6,611. The narrowest complete fixture
adds less than one millisecond of layout work while preserving its columns.
Raw runs and medians: `/tmp/carlos-compatible-refresh-ueg61wti/performance/` and
`/tmp/carlos-parent-final-dependency-perf-5wnn7u4a/results.json`.

The final reviewed Linux pair is staged at
`target/releases/carlos-table-compatible-d00a4c1c/`, with 402 notice files and all
409 file checksums verified. Archive:
`target/releases/carlos-table-compatible-linux-x86_64.tar.gz`, 45,284,987 bytes,
SHA-256 `96e3e0bde8234d1a1709f1b2e5e52a47c2d7b79df59dff603a45db943728c735`.
The package keeps the unchanged helper, build manifest, notices/provenance, and
relink bundle. `package-result.json` records those hashes/counts under the refresh
artifact root. No source behavior change followed the final green runner.

Parent's independent full release suite passed against the stable graph:
`target/ui-artifacts/run-BTpZDi`, with the same 483 Rust/one ignored, 51+51 helper
tests/272 assertions each, package smoke, 117 terminal cases, Codex contracts and
three native monitors, and four native Claude cases. Parent's final source,
lock, notice, and performance review has no open blocker. No additional broad
rerun is required. Parent then returned PASS and authorized the install recorded
below, preserving backups and the identical regular helper. Foreign platform
runtime jobs remain unexecuted locally; their resolved notice graphs pass.
No remote model call, new agent, commit, push, or release publishing occurred.

## Parent PASS and authorized install

Parent returned final PASS after independently checking the complete release
suite, table source/tests and native visuals, manifest ranges and larger-version
report, notice provenance, archive/checksums, and alternating performance. The
report now says base64/shlex direct version-range changes are deferred; it does
not assume those upgrades would require source changes. Parent explicitly
authorized the local install without another user confirmation.

The complete reviewed package is durably preserved at
`/home/wegel/.local/lib/carlos/releases/0.1.0-table-compatible-d00a4c1c/`.
All 409 packaged file checksums passed before copying it. The prior regular
Carlos executable is durably backed up at
`/home/wegel/.local/lib/carlos/backups/carlos-before-table-compatible-20261111T033219Z-cd12cd6e`,
with SHA-256 `cd12cd6e99ac7db84a2f0e1d027638c8ab48ad3eb07ecfced8008eb6c1c5bd87`.
Earlier backups remain intact.

The new Carlos was staged as a regular file, synced, and atomically replaced
`/home/wegel/.local/bin/carlos`. It remains a regular executable with mode 0755
and reviewed SHA-256
`d00a4c1c31da9e4a35a1d9517671b1ec0ec3a080ae33749c640385af6ebe2ba1`.
The regular sibling helper was left untouched: its hash, inode, and modification
time all match the pre-install values. Native Claude's executable hash also
remained unchanged; no user login or settings were altered.

Focused installed checks passed:

- `--help` through the actual regular bin executable.
- Both new table terminal workflows through the actual installed path, with helper override unset: `/tmp/carlos-table-compatible-installed-ui`.
- Parent's native Claude table probe through that same installed path, with isolated homes/config, fake key, and localhost only: `/tmp/carlos-table-compatible-native-233cag8r`.
- Native proof asserts fixed three-boundary column positions, full final cell contents at 120/54/36 columns, exact successful SDK fixture result, one local API request, actual Rust/helper process paths under `~/.local/bin`, and no helper override/development fallback.
- Parent independently verified the regular installed hashes and the native table visuals in `/tmp/carlos-parent-installed-table-final-5cyhcb5h`.

Durable install metadata lives in `~/.local/lib/carlos/current-install.json` and
`~/.local/lib/carlos/install-history/`; workspace copies are
`target/release-checks/table-compatible-install.json` and `local-install.json`.
The installed table fix and compatible refresh are complete. No additional broad
rerun or behavior change followed the reviewed checks. No paid/live inference,
new agent, commit, push, or release publishing occurred.
