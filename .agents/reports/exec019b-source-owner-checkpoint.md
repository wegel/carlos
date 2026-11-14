# Source owner and bounded painted rows

This checkpoint belongs to EP019b. It does not finish live large-source support. The formatter modules remain test-only; the terminal still uses its earlier source document path.

The renderer now accepts a heading plus an interval of painted rows and an exact total count. It checks whether the requested interval is present instead of treating any saved rows as the complete body. Existing whole-row callers keep their old behavior. The new complete-body formatter preserves each old body choice, role, heading, marker, fold, source-file colors and diff fallback tint. Markdown keeps whole-document references and tables. One worker owns the original and parsed/indexed body; a four-slot channel and one pending64KiB piece bound accepted-source transfers. Independent job numbers reject stale results even when a replacement pure cache reuses its token counter.

Focused tests cover every body format and full block at multiple widths, exact bytes after the suffix cache evicts the prefix, saved millionth-row addresses, source/width changes, retirement and queued old results. A held-owner test also fills the queue and verifies that poll alone eventually sends the latest viewport, with no further source traffic. The worker initially lacked this retry; it now pumps pending requests after draining replies.

The first focused command used a filename filter and ran zero tests. Do not count it as evidence. The subsequent complete isolated no-default workspace run passed1305tests with8ignored; all seven formatter tests appear by full module name in /tmp/carlos-ep019b/formatter-owner-isolated-workspace.log. Strict no-default Clippy passed in formatter-owner-isolated-clippy.log. The independent reviewer reran all seven formatter, three row-window and two Markdown document tests. The default release gate passes1338Rusttests with9ignored,60daemon tests,220terminal workflows,67compiled-helper tests and15architecture tests, plus strict lint and package smoke checks. Artifacts: target/ui-artifacts/run-SIRDFX in the isolated checkout; raw log: /tmp/carlos-ep019b/formatter-owner-isolated-release.log.

Root measured earlier whole Markdown and long-line helpers separately in the existing reports. This checkpoint does not supply new live latency figures or accept the known64MiB Unicode optimal-fit temporary memory peak. Copy, selection, live source backpressure, incremental append and socket lifecycle still need the actual terminal path. The next patch must connect and exercise that path rather than count another offline prototype as completion.

Eight alternating default-feature perf-session pairs on CPU2 compare the earlier accepted default release with this checkpoint. Both produce6001messages,57651rows and6000relevant items. Median per-run p95 before/after: scroll0.635/0.640ms, typing0.370/0.370ms, append0.410/0.405ms. This smoke comparison checks the shared renderer, not the test-only actor. Frozen binaries and SHA256values, all raw runs and records are in /tmp/carlos-ep019b/source-owner-perf.json. No actor speedup is claimed.

The size check finds all root functions/files below their caps. The accepted task painter predecessor has a61line wrap_natural_by_cells function; the task worker is extracting its existing hard-wrap loop in its final task commit. Do not attribute that pending predecessor fix to this proof.

## Engineering review

Reviewed the scoped diff against HEAD, including paged row addresses, formatter ownership, bounded source queues, stale-result checks, viewport retries, and row/style parity.

- **Findings:** No BLOCKER, MAJOR, or MINOR defects found.
- **Checks:** All 12 focused tests passed: seven formatter, three row-window, and two Markdown document tests.
- **Corrective guidance:** The supplied poll-test log ran zero tests. Use the `formatter` filter for future runs.

This verdict covers the prototype checkpoint. It does not approve live terminal support or the known long-line peak memory cost.

**Verdict: PASS**
