# EP019b second whole-roadmap review

Fable 5.2 (`claude-fable-5-2`), medium effort, read-only, 2026-11-14. Session `7064946e-dce3-4b34-b622-e2b49e65b281`. This is an interim review of the now-real socket consumer, not EP approval. The reviewer read plans/code and ran no tests or commands.

Review complete (read-only; nothing run or edited). No approval of EP019b.

## Overall judgment

The client/replica/cursor layer is sound and lean. The weak part is the join between windowed sources and the render cache, and the mmap file does not address it. I would not build the file fallback next: it is premature, though not wrong. Rewriting the renderer to stream Markdown is not justified either.

## Ranked risks

1. **Blocker for deletion: the render cache needs every row's source at once.** `rebuild_rendered_blocks_from` recounts every row from `dirty_from` to the end through `input()`. A row whose source is not loaded counts zero lines and disappears. Tool rows are never inline (`event_rows.rs:32`), so the 33rd tool row hits "terminal source windows are full" (`remote/bodies.rs:47`) and vanishes. After a reset only the last 32 rows are even observed (`events.rs:79`). This is the common case, and it hurts more than above-cache bodies. Eviction cannot work until counts and plans are kept per entry (id, revision, width, mode) rather than as a positional suffix.

2. **Blocker: mmap does not bound what actually grows.** `materialize_block` renders the whole block into owned `Vec<RenderedLine>` (text plus styled segments), which is plausibly at least twice the source size. Every append or width change also re-parses the whole document. A mapping removes one source copy and leaves both of these.

3. **Blocker: the scroll anchor is captured after the prepend.** `scroll_anchor` runs in `draw`, after `receive` has applied the history page, so `entry_at(index)` resolves the stale index to a different entry. Store the anchor as `(EntryId, row)` in the viewport at the end of each draw and whenever scroll changes. That is simpler than capturing before every apply path.

4. **Major: rows blank while descriptor and window disagree.** `from_client` requires exact equality and `document()` returns nothing while `has_more`. A live row therefore disappears between the entry event and its chunk. A Unix socket batches these together; EP020's WebSocket and EP022's phone will not.

5. **Major: failed reads are permanent for a revision.** `Read.failed` has no retry path once capacity frees. With eviction, capacity failure becomes routine, not exceptional.

6. **Minor: resize needs all sources again.** A width change invalidates every count. Without a policy this means re-reading the whole transcript over the wire on each phone rotation.

## Recommended path

1. **Key the render cache by entry.** Keep each row's count and plan across source eviction. Invalidate on that row's revision or on width/mode, and rebuild offsets as a cheap prefix sum. Request source only for rows near the viewport or needed for copy.
   - For rows never yet counted at the current width, show the last-known or estimated height, visibly marked as loading. That is not a silent drop, because the entry-id anchor keeps the view stable.
2. **Evict least-recently-visible windows** inside the existing 8 MiB / 32-slot budget. Draw the loaded prefix with a marker rather than nothing.
3. **For above-cache bodies, add a validated pass-through in `carlos-client`.** `Bodies` checks cursor, revision and offset, then hands the chunk to a platform sink without retaining it. The crate stays pure, and the terminal and later iOS can each choose storage. Start with one pinned heap document for the visible or copied row, released on scroll-away. That matches what the embedded path holds today and adds no second copy.
4. **Add the file and mmap behind that sink only if measurement shows it matters.** The conditions would be:
   - an unlinked private file under the XDG cache dir, never `/tmp`, which is often tmpfs and therefore RAM;
   - mapped read-only, with a remap on growth;
   - no `&str` held across `service()` (true today: `RenderCountCache` is local and plans own their data, though I did not inspect `BlockBody`);
   - UTF-8 already guaranteed by the chunk boundaries.
5. **If rendered lines dominate, window materialization inside a block.** Line-to-byte checkpoints plus SGR state would cover plain, ANSI and diff bodies. Markdown stays whole-document, because reference definitions make exact streaming infeasible.

## Proof to do next

- **Many-row terminal test:** 200 tool rows plus history paging. Assert every row is visible after scrolling, the window count stays at or below 32, and the anchor is stable across a prepend and a resize.
- **Release measurement:** 64 MiB ANSI and 16 MiB Markdown bodies. Record VmHWM split into source and rendered block, per-append p95, and resize p95. This decides steps 4 and 5.
- **Exact copy of an above-cache body** through the pinned path.
- **Live append under eviction pressure:** no blank frames and no repeated subscriptions.

## Maintainability defects (separate)

- **Duplicated lifetime state:** `DocumentReads` mirrors `Bodies` (revision, liveness, a literal `32` versus `MAX_WINDOWS`). The two drifting apart already caused the MAJOR finding in the first-consumer review.
- **Document mode is a flag in three places:** token, `Window` and the `apply` argument, with the document stored as one segment in a `VecDeque` of `Pieces`. The pass-through sink would remove this.
- **Name shadowing:** `struct Read` in `remote/bodies.rs` shadows a common trait name.
- **Over-wide invalidation:** `dirty_entry` on every 64 KiB chunk marks the whole suffix dirty.

## Codex assessment and next work

I accept the common-case source/cache findings and the work order. The renderer currently drops an unloaded row to zero height and recounts a positional suffix. The 32-source limit therefore blocks ordinary tool-heavy sessions before an oversized-body fallback becomes useful. Build a 200-tool actual-terminal proof, keep plans/counts by stable entry and format, show a visible loading state, and evict source windows by visibility under the existing count/data limits. Preserve a loaded append-only prefix while newer bytes arrive; never present it as a completed whole source. Record a stable entry/row anchor when the viewport is current, before applying prepended history, rather than resolving an old cache index against new facts.

Add an explicit retry after capacity becomes available; do not remove the failure fence and revive a repeated-watch loop. Consolidate duplicated read-lifetime decisions with Bodies as this actual consumer grows, rename the ambiguous Read type and remove per-chunk suffix recounts. Keep all existing cursor/revision/offset checks. These are required fixes before deletion; they do not require a human choice or stop the authorized work.

I accept deferring both file/mmap storage and a streaming Markdown rewrite. Neither fixes whole-block materialization or rows disappearing after source eviction. After the many-row consumer works, add a validated pure-client pass-through only when the actual above-cache caller needs it. One pinned original heap document is a candidate to measure, not an accepted new memory guarantee. Measure 64-MiB ANSI and 16-MiB Markdown with the actual renderer, split source and rendered-block high-water figures, exact copy, append and resize p95. A file/mapping fallback or within-block checkpoints should follow that evidence. No new file worker, storage mechanism, cache bound or product body cap is approved by this report.

The local display/clipboard slice remains separate and has passed its restored 191-workflow gate and engineering reviews. EP019b is still incomplete. Its final whole-roadmap review must include actual complete socket parity and the separate embedded-path deletion.

