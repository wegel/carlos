# EP019b: keep unloaded transcript entries reachable

This report covers one terminal cache/history slice on `iphone-spike`, starting at `53bee52`. EP019b remains incomplete. The default terminal still owns its embedded backend until the separate cutover/deletion gate; this slice changes the opt-in `--daemon-socket PATH resume SESSION_ID` consumer and the shared renderer it uses. No EP approval, install, merge or push belongs to this report.

The terminal now retains an entry's wrapped height, parser choices and painted rows separately from its original-source window. Every saved memo carries its stable entry id, source revision, display width/mode and neighboring heading facts. A missing source gets a visible loading row and a nonzero estimate, or retains the last accepted raster while newer bytes arrive. The terminal never feeds a suffix or placeholder to the Markdown/ANSI parser as a complete original source.

The cache keeps memos in chronological order. A normal append or source change reuses that order directly; only a prepend/reorder builds a temporary lookup by stable id. Each memo owns one BlockPlan; only painted line vectors need shared ownership. A typed section kind/failure flag replaces repeated heading strings/colors. The renderer resolves those display values when it draws a heading. Both vectors reserve ordinary geometric headroom so a first live append does not move every cached entry. Known-new rows skip invalidation; a dirty earlier prefix keeps the id fallback available while facts have moved ahead of the last drawn index.

The terminal requests complete original sources needed by visible rows, evicts unused source windows before opening another, and sends explicit unsubscribe requests. It shares the pure client's allowance of 32 windows and eight MiB data allowance rather than adding a new body budget. It charges cached descriptors as well as row descriptors, so tool sources count even though tool metadata omits Entry.source. Before a source notification or first reply reaches Client.receive, the terminal may use its small address/size hint to release unused cache data. The pure client still checks request/token ownership, revision, cursor, offsets and all text bounds before accepting any bytes. This preflight does not turn unvalidated bytes into source text.

F6 clears failed source-read latches and retries needed visible sources. Ordinary live appends never reopen a failed watch. The terminal saves a stable entry/row anchor after drawing and after local scroll input, while facts and the drawn index agree. Home/PageUp/upward wheel at the loaded prefix sends one older-page request; the saved anchor keeps the old prefix visible after that page arrives. A later scroll can enter the older page. Failed history reads require another explicit scroll action, and F5 retains its existing reconnect behavior.

## Actual terminal and daemon checks

The new checks use the actual Carlos process, actual carlosd Unix socket and deterministic native Codex peer. The relay records both sides, may hold source bytes while numbered facts advance, or may drop one numbered append to force a genuine client gap and snapshot. It supplies no invented replacement snapshot.

- 200 tool outputs: the daemon's first snapshot carries100 rows. The terminal requests one older page, preserves the old prefix anchor, pages through all200 unique original texts, stays within32 simultaneous source watches, resizes to28 columns and returns to the evicted oldest source. Full output is pinned explicitly so the phone's normal automatic compact mode does not change this test's purpose.
- Delayed live bytes: newer numbered source facts arrive while the relay holds their source update. The accepted prefix remains on screen beside a loading notice; the composer accepts a private draft. Releasing bytes shows the new tail without another subscription.
- Explicit retry: an injected first source-read error stays stopped across three real same-revision native appends. F6 retries that visible source, clears the hint and preserves the draft, with no chat send.
- Same-id reset: dropping one native append creates a numbered gap. Carlos requests an actual daemon snapshot and displays the new inline text under the exact same entry id.
- Empty then nonempty: Carlos reads a65536-byte whitespace answer, counts it at zero, evicts it while paging81 tools, then receives nonblank text under the same source revision. Scrolling home reads it again and shows the new text. The test waits for the terminal's numbered event and scrolls again if the append arrived during the first Home key.
- Live byte pressure: an unused 6 MiB answer and a visible 1 MiB answer share the cache. Growing the visible answer beyond 2 MiB evicts the unused body before admission, shows the new tail without F6 and retains the draft with one source subscription. A separate variant puts the unused 6 MiB body in a tool; its cached descriptor must count too.

The 13 socket-terminal checks pass. Each prior engineering defect also has a negative proof: remove only reset invalidation, the stale positive estimate, source-growth preflight or cached-tool byte accounting; the corresponding actual UI test fails. The script restores exact source in finally and rebuilds. Raw logs/results are `/tmp/carlos-ep019b/sparse-negative-final-{reset,zero,growth,tool}-ui.log` and `sparse-negative-final-results.json`; captured screens/RPC are under `target/ui-artifacts/ep019b-sparse-negative-final-*`. The normal source tests live in `tests/ui/test_daemon_{client,history,source_recovery}.py` and run through `scripts/test.sh`.

The first recovery fixtures incorrectly appended deltas to completed native-history items. Carlos correctly ignores those late deltas, so the final fixtures publish unfinished native items. A pathological multi-megabyte single word also stalled a source-read proof long enough for the daemon to disconnect its slow subscriber. The byte-pressure check uses ordinary fenced rows to isolate cache admission; that long-line cost remains relevant to the large-body renderer gate below.

## Matched release measurements

The exact final source uses explicit default-feature releases, private config/state homes, CPU6,160x48 and 15 rotated paired rounds. The original pre-EP016 binary, accepted EP019a binary, pre-slice53bee52 binary and new source remain separate. `/proc/PID/exe` must identify the executable before sampling VmHWM; Python's inherited wait4 high water and the harness's zero RSS field are excluded. Raw data, identities and paired bootstrap intervals are `/tmp/carlos-ep019b/sparse-accepted-latency/`.

- preplan: `c880c65cc8732b296744bca7f42be5f729ab8b78580c84f2c95113736a1c0b53`, 19,346,328 bytes, `/tmp/carlos-base/target/release/carlos`.
- ep019a: `f03799c8ef130bc5967b6eb9cb24abf3aca2e312d747a4426281ec674711f979`, 20,346,176 bytes, `/tmp/carlos-ep019b/ep019a-carlos`.
- pre_sparse: `869292a589a3d048c36af480821145f7ae10b2419e62a6cc4c52aef5fad1517f`, 21,768,792 bytes, `/tmp/carlos-ep019b/local-navigation-carlos`.
- current: `a32f2fa5a594f2b0da91bbc0941f1c0af7f2f87c9fe90980070f1e3b2361f8a1`, 21,799,680 bytes, `/tmp/carlos-ep019b/sparse-accepted-carlos`.

Synthetic counts stay 6,001 messages/39,347 rendered rows/6,000 items. Median layout/replay, scroll/typing/append p95 in ms and sampled VmHWM in KiB follow, in that order:

- preplan: 14.98/47.47; 0.83/0.63/0.66; 23984.
- ep019a: 14.90/46.91; 0.82/0.63/0.68; 24932.
- pre_sparse: 14.98/47.19; 0.82/0.62/0.67; 27044.
- current: 14.92/47.20; 0.82/0.62/0.67; 25532.

The final paired synthetic append-p95 differences have intervals spanning zero against all three baselines. The first hash/Arc prototype added about2ms to initial layout; the ordered cache removes that regression. The later first-append cost disappears after known-new rows skip scanning and vectors retain geometric headroom. Against53bee52 the final synthetic sampled high water falls by 1,512 KiB. This process measure includes linked code and other memory, not just cache allocations.

The saved-session fixture preserves 5/9/4 counts. Final median replay is 0.99 ms, scroll/typing/append p95 0.45/0.40/0.48ms and sampled high water 12616KiB.

A separate seven-round paired test checks append cost at 250 and 4,000 turns against53bee52; raw data are `/tmp/carlos-ep019b/sparse-accepted-scaling/`.

- 1501 entries: append p95 0.69ms versus 0.68ms; paired median +0.01ms,95% interval [+0.00,+0.02]ms.
- 24001 entries: append p95 0.73ms versus 0.71ms; paired median +0.04ms,95% interval [-0.01,+0.06]ms.

These figures show no growing full-cache append scan. They exercise the still-embedded perf-session adapter. Whole-socket/all-process speed and memory checks remain required at the actual cutover.

## Reviews

### First engineering review

Reviewed the uncommitted source-reader/render-cache changes against `53bee52`, the specified UI tests, and directly called Client interfaces. I made no edits. The targeted retry log passes; I did not independently run the full suite or pending performance checks.

1. **MAJOR: A reset can leave old text on screen indefinitely.**  
   [events.rs:66](/var/home/wegel/work/perso/carlos-daemon/src/app/remote/events.rs:66) rebuilds the chronological index but does not invalidate saved entry rows. [render_entry_cache.rs:100](/var/home/wegel/work/perso/carlos-daemon/src/app/render_entry_cache.rs:100) then accepts an unchanged key, generation, format and neighbor as sufficient to reuse old text. For example, a reconnect snapshot can extend an inline answer under the same entry ID and leave its generation as `None`; Carlos continues displaying the previous answer. An evicted source that grows under the same revision can similarly retain an apparently current raster without requesting fresh bytes.

   Invalidate affected entry memos on reset, or compare the current text, source descriptor and display facts before reuse. Preserve the accepted raster while marking it stale. Add a reconnect/reset test with changed text under the same entry ID.

2. **MAJOR: A previously empty source can remain permanently invisible after eviction.**  
   [render_entry_cache.rs:193](/var/home/wegel/work/perso/carlos-daemon/src/app/render_entry_cache.rs:193) preserves a cached zero line count even when the entry becomes stale and its source is unavailable. The cache then excludes that entry from source requests because `needs_source` requires a positive count. A blank or whitespace-only tool source that is loaded, evicted, and later receives nonblank text under the same revision therefore cannot load again through normal scrolling.

   Give stale, unavailable entries a positive estimate unless the current display explicitly hides them. Add a test that loads a blank source, evicts it, appends visible text, and verifies both the new read and the screen.

3. **MAJOR: Existing watches bypass byte-budget eviction when their sources grow.**  
   [bodies.rs:50](/var/home/wegel/work/perso/carlos-daemon/src/app/remote/bodies.rs:50) skips `make_room` whenever a current source window exists. If unused windows retain 7 MiB and a visible source grows from 1 MiB, the Client rejects the next chunk for exceeding the shared allowance. Its watch handler closes that source, and `DocumentReads` latches the failure. Carlos stops updating the visible answer despite having unused windows it could evict.

   Apply byte-budget eviction to growing windows before admitting their new bytes, while keeping genuine source failures latched. Add a terminal test with unused retained bodies, a growing visible source, and assertions that Carlos evicts unused windows and displays the new tail without F6.

**Verdict: FAIL.**

### First follow-up

Reviewed the three repairs, compact heading facts, reserved map capacity, directly called Client checks, and supplied test/performance evidence against `53bee52`. No edits made.

- Reset and zero-height repairs address the prior findings.
- I found no defect in the smaller heading cache or capacity reservation.
- **MAJOR: Byte-budget checks still count retained tool sources as zero.** In [bodies.rs:126](/var/home/wegel/work/perso/carlos-daemon/src/app/remote/bodies.rs:126), `make_room` totals bytes through `source_bytes`, which reads only `entry.source`. Tool entries omit that field, although their cached source windows retain bytes.

  With an unused 6 MiB tool source and a visible answer growing from 1 MiB past 2 MiB, Carlos therefore skips eviction. The Client rejects the next chunk, closes the watch, and Carlos marks the read failed. The new growth test misses this because its unused source is an answer.

  Count cached source-window bytes or descriptors for every retained read, including tools. Add a variant of the growth test with the unused 6 MiB body supplied by a tool; assert eviction and continued updates without F6.

The negative proofs support the three tested cases. I did not independently rerun tests; the restored full suite was still running when inspected.

**Verdict: FAIL.**

### Ordered cache follow-up

Reviewed the uncommitted source/cache changes against `53bee52`, including byte accounting, reply checks, ordered reuse, and removal paths.

The prior **MAJOR is resolved**: `make_room` charges cached tool descriptors. I found no new correctness defect in ordered reuse or suffix retirement.

- **MINOR: New entries trigger a full cache scan.** [render_entry_cache.rs:117](/var/home/wegel/work/perso/carlos-daemon/src/app/render_entry_cache.rs:117) scans every retained entry when the requested position is absent, including ordinary appends. The embedded caller invokes this for each new message, so appending *k* messages after *n* cached messages adds O(k×n) comparisons without any reorder.
  
  Have callers skip invalidating known-new entries while preserving the fallback for moved entries. Check append cost across increasing transcript sizes.

I checked the reported 13 passing terminal checks and four expected negative-test failures. I did not rerun tests; the final full suite remains unverified by this review.

**Verdict: PASS WITH ISSUES** for this slice.


The first two FAIL verdicts caused the fixes and negative proofs above. The ordered-cache reviewer accepted the fixes and found the known-new invalidation scan as MINOR; that scan is now removed. A final narrow follow-up and the latest full test-run details are appended below when complete. This is still no EP approval.

## Remaining EP work

The32-window/eight-MiB bound applies only to retained original-source data. This slice does not bound all painted rows or admit an above-cache product limit. EP019b still needs64MiB ANSI/16MiB Markdown measurements, exact above-cache copy, append/resize high water and p95, then the justified storage/within-block choices. Fable's prior advice still governs: prove a validated pure-client pass-through with an actual caller; measure one pinned document before adding a file/mmap worker; keep whole-document Markdown semantics. Full startup/panel/settings/dictation/command parity, separate embedded-path deletion, final engineering/Fable whole-roadmap reviews and delegated approval remain unfinished. Main and the installed binary remain unchanged.


## Final narrow reviews

### Known-new-row follow-up

Reviewed the two fixes and directly called `EntryMemos` methods. The socket append guard and vector reserves look sound.

- **MAJOR:** [render_cache_state.rs:134](/var/home/wegel/work/perso/carlos-daemon/src/app/render_cache_state.rs:134) can skip changed embedded messages after a prepend. Cache `[A, B]`, then supply `[X, A, B′]` with the same IDs, changed B text, and dirty index 0. The loop invalidates only X and A. `prepare` can reuse B’s old count and painted text because embedded messages have no revision and B’s preceding context still matches.

  Invalidate the old cached suffix by its old positions whenever `start < rows.len()`. Pure appends can still skip that work. Add a regression test that prepends X and changes B before the next draw, then checks B’s text and line count.

I did not rerun tests. The available append summaries show similar p95 times at 1,501 and 24,001 entries; they do not cover this correctness case.

**Verdict: FAIL**

### Old-suffix repair follow-up

Reviewed the two cache methods and the new regression.

- **Findings:** None. The repair marks the saved old suffix stale before entries move. Pure appends start at the old length and visit no saved entries.
- **Corrective guidance:** None needed.
- **Validation:** The targeted Rust regression passed, checking cached text and row counts against a fresh cache in Gutter and Flow at 12, 28, and 80 columns. The supplied negative log confirms it failed before the repair.

**Verdict: PASS** for this narrow repair. Full-run and final performance checks remain outside this verdict.

The final legacy regression failed before the repair with 8 cached rows versus 16 fresh rows. The embedded adapter now marks the saved old suffix stale, independently of the new facts' positions; a pure append starts at the old length and visits no saved entries. The socket path keeps its id-based changes and known-new guard. The final source also clears old visible interests when no transcript row is visible and ignores another session's source notifications during preflight. Final workspace/strict lint/full terminal/release evidence follows; no known reviewer finding has been deferred.


## Accepted slice evidence

The exact final source passes 824 main Rust tests (three ignored), 58 pure client tests and all other workspace suites, strict all-target Clippy, explicit release build, 67 helper checks and 15 architecture checks. The full actual-process runner passes 49 daemon and 197 terminal workflows; artifacts are `target/ui-artifacts/run-wYMWR7`. Logs are `/tmp/carlos-ep019b/sparse-accepted-{full,clippy,release}.log`. Every changed Rust module stays below 400 lines and every changed function stays at or below 60 lines. The final old-suffix reviewer verdict is PASS; all earlier MAJOR findings are fixed.

Main still points to `a29b1a1e3720e48b09e2ca984fb16c29b5857ea3`. The installed Carlos SHA remains `d4595e2ac79f9ccb4cf279f08fc5dfc903e98aaec21960d7d0dc6c4727f6b2a3`. No EP is approved or archived here. The next work remains the measured large-body consumer and complete terminal migration described above.
