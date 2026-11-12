# Milestone 9.2: native task readers and local panel focus

The session owns bounded agent/process snapshots, verified owners, partial child reads, native stop requests, cached reports, process-scope errors and targeted task refreshes. The view owns only panel visibility, selection, scroll and detail mode. Every native record exposes borrowed reads; only session commands change its fields. Plain PanelScope values borrow the user's explicit target for one call and carry visibility for polling; the session never stores those client flags.

The off-thread reconnect panel reader now constructs SessionState directly. It uses the same bounded native reader and never constructs AppState, a TextArea or client preferences. `src/codex_snapshot.rs` calls it directly. The old app reader and pure panel/page/activity/task-page files were deleted after their callers moved.

## Handler audit

Panel tick/wait keep enabled/closed and visibility checks, older/detail-window rules, the three-second refresh interval and original whole-read deadlines. Request tokens still use the shared saturated counter. Outgoing work retains only live snapshot/activity/stop tokens, and continuation pages keep their token and start time. Expiration discards the same pending work and retains prior complete rows/items.

Snapshot pages keep all byte/count/cursor/lineage limits: eight MiB, bounded agents/processes, unique ids/cursors, a bounded verified parent chain and exact root/epoch ownership. Native failures retain the same previous rows; unavailable process scopes remain explicit errors. Publish installs only a complete read and offers the first native identity only when the client had no selection. A selected identity stays pinned when its row disappears.

Child item reads retain exact item/turn deduplication, the 128-item and eight-MiB bounds, the 64-KiB live-record limit, and the UTF-8-safe last-16-KiB live tail. Child metadata/events still run before the root filter. Missing/foreign/unverified children follow the same returns; malformed/oversized status or turn data returns before updating activity. Parent checks, report sizes and aggregate metadata budgets are unchanged.

Stop still requires an open backend, no prior stop and the exact selected current native identity. An agent stop first reads the latest native turn, then sends interrupt only for a still-present target and an in-progress valid turn. A process stop uses the original owning thread and process id. Replies match root, epoch, request token, method and exact params; a stop acknowledgment never invents completed task state. Failed or malformed replies retain the same error/refresh behavior. Selection changes discard only the previous selected-child read.

Targeted task refreshes also refuse reads during a rebuild and keep generation/root/epoch/token/revision fences, owner checks, page/deadline/cache bounds, rate limits and no-idle-poll behavior. Complete process pages replace only their owner. Empty success clears that owner's old uncertainty/error; another failing owner remains uncertain. Termination retires only the exact thread/process/item tuple. The task badge counts the same verified active/error agents, child callbacks and authoritative native processes independently of panel visibility.

Closure, root switch and reconnect installation now call named session commands and preserve the old fields/reset order. The moved handlers run on the same threads as before. No native decision crossed a producer/consumer boundary. The report applies local selection/detail effects synchronously before another input event can run.

## Checks and direct review

All 997 Rust tests pass, including all existing panel bounds, deadlines, stop, task ownership, stale-reply, count, reconnect and editor-retention fixtures. Test setup now uses narrow session fixture methods; existing screen/request expectations remain intact. A new headless test reads a complete reconnect panel from two fixed native replies, asserts the exact process-list request and resulting facts, and consumes no terminal fixture. All 174 terminal workflows pass in 77.513 seconds.

Release build, format, all-file style, whitespace and size checks pass. Files stay at or below 393 lines and functions at or below 60. Private helpers were split by native scheduling, event acceptance, item retention and agent-stop concerns. Unused context parameters were removed rather than carried as a compatibility layer. The sole compiler warning remains the pre-existing protocol_writer atomic rename.

The primary agent directly checked each moved guard, default, failure path, cursor/byte bound and stop identity against parent `8755c39`, using the engineering reviewer prompt. Verdict: PASS, no BLOCKER or MAJOR. No subagent or independent reviewer ran.

All 3,000 parent comparison streams match exactly (51,390 checkpoints); neither endpoint panics. Pre-plan output equals part 9.1 byte for byte, including the same 103 classified differences. This inherited corpus checks common transcript/callback/history effects, not exhaustive panel-page schedules; those native-reader cases remain covered by the enumerated Rust and terminal tests. Speed figures and artifact paths are recorded in the ExecPlan.

Additional source callers only read moved facts or apply local fields: task-badge/status rendering, history report clearing, root switch, reconnect install and keyboard/mouse panel routing. No protocol schema, dependency, persistent format or installed binary changed.
