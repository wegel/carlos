# Wrap Markdown cells inside their columns

## Scope and working rules

The user reported long Markdown cells escaping their table columns. Preserve the
reviewed, uncommitted Claude migration and the installed regular executable pair.
One implementation agent works sequentially; parent reviews before any install.
Do not commit, publish, spawn agents, or call a remote model for this task.

## Progress

- [x] Read the existing draw/count paths and pinned tui-markdown source.
- [x] Preserve installed baseline at `/tmp/carlos-before-markdown-table-reflow`.
- [x] Parent checked canonical upstream and found the existing cell-width fix in release 0.3.13; replace the fork and discard the custom normal-table engine.
- [x] Give upstream the available width and keep reasoning, row counts, styles, selection, and resizing consistent.
- [x] Add eleven focused Rust regressions and two actual-terminal workflows; focused checks pass.
- [x] Parent reviewed the focused upstream patch and returned PASS, then assigned the explicitly selected compatible crate refresh.
- [x] Final combined release/style/lint/performance checks after the compatible refresh passed; evidence lives in ExecPlan 011.
- [x] Parent final review returned PASS after compatible refresh; the authorized regular-file install and focused installed table checks passed.

## Surprises & Discoveries

The original tui-markdown revision `819cbca7675178947357a88e9e4a007bfb085ccf`
computes each column from its longest cell and emits one line per row. Its public
options have no width, and its public Markdown blocks flatten tables to text.
Carlos then wraps entire formatted rows as ordinary text. Reasoning currently
parses one source line at a time, which also prevents multiline table parsing.
The agent initially checked only that unchanged fork and began a custom table
layout. Parent then checked canonical upstream and found the exact width-aware
fix, introduced in commit `3b69efa` and released in tui-markdown 0.3.13 (release
commit `9c4fcaa6888ba32f03ce3a54fd21991e820b3c7f`). The custom grid, cell wrapper,
inline-style collector, and comment anchors were discarded. Carlos now pins the
registry release `=0.3.13`; the lock selects pulldown-cmark 0.13.6, satisfying its
0.13.5 minimum. This dependency change removes the fork's runtime test-only
dependencies as well. Future checks must compare the canonical project when a
fork has stopped moving.

Upstream accepts `Options::table_width(u16)`, counts borders/padding and enclosing
list/quote prefixes, and wraps/pads styled grapheme cells. It documents one limit:
if even one grapheme per column cannot fit, its grid exceeds the requested width
to keep content. Carlos handles only that impossible minimum with labelled
paragraphs. Parser-owned cell/source ranges retain escaped pipes and inline
syntax; the whole source still goes through upstream so references keep working.
The stylesheet now hides code fences and heading markers through public methods.
Upstream adds no numbered code gutter, so the old stripping workaround is gone;
the shared RTK gutter detector remains. Carlos never enabled the fork's image
terminal feature or used its private rich-block model.

Parent reproduced the installed bug using native Claude and a localhost fixture:
`/tmp/carlos-parent-table-before/narrow-54.txt`. Borders wrap over three rows and
cell text escapes its columns. Parent probe: `/tmp/carlos-parent-table-probe.py`.

## Decision Log

- Use upstream 0.3.13 for normal table cells, styling, alignment, borders, and width allocation. Do not maintain a second grid engine.
- Use the same structural parser options as upstream, without unrelated smart punctuation. Rewrite only a table whose borders/padding/minimum graphemes plus native list/quote prefixes cannot fit. Show complete header-labelled rows instead of splitting a formatted grid.
- Share the width-aware Markdown path between count and draw. Parse multiline reasoning tables as a document; retain the cheap per-line path for ordinary reasoning. Preserve physical newlines when selecting/copying table rows.
- Keep the installed helper unchanged. A later approved Carlos install must remain a regular executable in `~/.local/bin`.
- The user selected compatible crate updates and a report of larger upgrades. Parent owns that audit and will assign the compatible refresh after reviewing this focused table patch. Run final release checks once against the combined scope.

## Validation and outcomes

The discarded custom renderer first passed six, then nine Rust cases and two
terminal cases. Both new terminal cases fail against the installed baseline in
`/tmp/carlos-table-reflow-ui-baseline`, reproducing detached borders and cell
contents. The final upstream patch passes eleven focused table Rust cases, including
8–99-cell draw/count equality for assistant and reasoning, Unicode/styles,
alignment/copy, escaped pipes/code/reference links, HTML/prose boundaries,
fenced-code exclusion, and impossible nested quote/list/Unicode-link budgets.
All 75 preexisting Rust UI-render cases also pass. The two upstream terminal
cases pass on the final debug build in `/tmp/carlos-table-upstream-ui-final`; their wire assertions verify
the exact two streamed fragments and one user turn. They cover 120/54/36-cell
grid reflow during streaming and an 18-cell labelled fallback followed by a
return to a desktop grid. Initial custom-layout expectations for narrow columns
and quote prefixes were corrected to upstream's documented minimum and `>`
prefix, without weakening column/content/count checks. A boundary fixture first
used a bare trailing word, which the Markdown parser correctly treats as another
table row; the test now uses a heading to create a real adjacent block. The
fallback explicitly preserves that boundary both outside and inside a quote.

`cargo test --all-targets --quiet` passes all 483 tests (one ignored), including
the eleven focused cases. Strict Clippy, rustfmt, present-file style, whitespace,
and new Python fixture syntax/400/60 checks pass on the focused upstream patch.
Final combined release checks and performance remain pending the parent's
next sequential task. No installed executable changed. Parent saved another
independent baseline in `/tmp/carlos-parent-table-baseline-8yd3mdop/`; its initial 1,000-turn
synthetic sample was replay 49.71 ms, layout 15.55 ms, scroll p95 0.86 ms, typing
p95 0.66 ms. Final comparisons will alternate old/new release binaries with the
same inputs rather than compare debug and release builds.
Parent also supplied a 200-message table-heavy fixture and 120/54/36-cell
baseline reports in that folder, to distinguish ordinary-text cost from changed
table layouts.

Parent independently passed all eleven focused Rust cases, both new terminal
cases in `/tmp/carlos-parent-upstream-table-ui`, and an actual native Claude
localhost visual probe in `/tmp/carlos-parent-table-after-debug/narrow-54.txt`.
The new screen has single-row borders and cell continuations inside their
columns. Parent found no blocking issue and returned PASS before the compatible
dependency task began. Final combined checks/install remain separate.

The combined table/dependency work later passed final parent review and was
installed as the actual regular executable at `/home/wegel/.local/bin/carlos`.
Both new table terminal workflows passed through that installed path in
`/tmp/carlos-table-compatible-installed-ui`. Actual installed native Claude with
a localhost fixture also passed 120/54/36-cell table column/content checks in
`/tmp/carlos-table-compatible-native-233cag8r`; parent independently confirmed
the same in `/tmp/carlos-parent-installed-table-final-5cyhcb5h`. ExecPlan 011
records exact package, backup, and install metadata. The helper was not replaced.
