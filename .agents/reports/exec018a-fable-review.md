# Review of the Carlos iPhone/daemon spike (`iphone-spike` @ f42d870)

Caveat: the `/tmp/carlos-fable-review/` files were permission-denied, so this rests on the docs, the EP018 report and source reads. I ran nothing; test and perf figures are the report's claims.

## 1. Overall: sound with course corrections

The foundations (EP016–018) are proportionate and I found no serious problem in them. The risk is in what EP018 did not prove and what EP019 assumes. Protocol v1 is sized for the phone (`tmp/main-screen.md` §11), but the roadmap says the TUI "becomes a client through `carlos-client`". The TUI is nowhere near being a v1 client, and the core still assumes exactly one synchronous client with a composer. Fix both before any socket code, or EP019 will bloat the protocol or leave two permanent client paths.

## 2. Concerns, ranked

**1. The TUI does not actually live on the event stream. (Decide before EP019.)**
- `src/app/core_events.rs:8-39` reduces events to one dirty index.
- Rendering still borrows the core transcript directly (`src/app/state_transcript.rs:63-69`).
- Non-test TUI code reaches into `self.session.` at about 377 sites in 68 files: Codex panel, goals, queue, settings, Claude tasks, rewind/edit, model settings, callbacks.
- `Command` has 11 methods (`crates/carlos-proto/src/commands.rs:38-84`), none covering those.

So EP019 must either grow the protocol by dozens of backend-specific facts, drop TUI features, or keep a privileged in-process path forever. Smallest correction: classify those call sites as in v1, needing a protocol addition, or purely local. Then scope EP019 as daemon, session thread, Unix socket and a headless v1 client. Move the TUI over only once the inventory is priced.

**2. The core has a second, client-shaped channel that cannot survive multiple clients. (First milestone of EP019.)**
- Handlers take the client's view state as input: `BackendContext { panel, callback_focused, claude: { composer, confirmation } }` (`crates/carlos-core/src/session/backend_update.rs:12-16`, `claude/receive_update.rs:14-17`), threaded through about 60 signatures.
- The core answers with editor commands: `SetInput`, `ClearInput`, `FollowBottom`, `RetainDraft`, `LoadRetained`, `CodexUnfocus` (`claude/receive_update.rs:29-51`).
- The core branches on `composer_is_empty` and `composer_matches` (`receive_update.rs:55-79`).
- `submit_turn_text` blocks up to 10 s on the backend (`session/prompt_commands.rs:129-135`, `175-182`), which would stall a session thread's publishing.

A daemon has no single composer. Correction: the core emits facts keyed by `client_message_id` (prompt rejected, edit failed), and each client decides what to do with its draft. `ClaudeEffect` and both context structs should then disappear. If they survive EP019, that is lasting avoidable complexity.

**3. v1 is "frozen" before any consumer exists, and has one design gap. (Now for the wording; EP020/021 for the gap.)**
- Appended tool text becomes `EntryFactsChanged` with no text (`session/events.rs:255-262`), and the client must refetch the whole body via `entry/output` (`docs/client-protocol-v1.md:53`). An expanded running command is O(n²) over the wire, against speed rule 2. Add an output-append event or an offset on `entry/output`.
- `summary`, `added`, `removed` and `client_message_id` are always null (`session/event_rows.rs:26`, `77-79`).
- `Approval.kind` is a free string while every sibling is an enum (`session/event_approval.rs:46-57`).

Call v1 a draft until the iPhone app has consumed it, and spend nothing on compatibility machinery before then.

**4. A hand-written Rust lexer guards a boundary the compiler now guards. (Delete now.)**
`scripts/check-session-boundary.py` is 386 lines of tokenizer, module-tree walker and use-tree expander, plus Python tests. It still carries pre-workspace branches (`core_files`, lines 105-113; `EXTERNAL_TESTS`). Since EP017, `carlos-core` cannot import the binary crate or an undeclared dependency. `scripts/check-core-dependencies.py` (86 lines over `cargo metadata`) covers the rest. Its only unique rule, where `SessionState` impls live, is already enforced by `pub(in crate::session)` fields.

**5. `SessionState` is a union of all backends behind a very wide surface. (Later; just don't feed it in EP019.)**
- It has 27 fields, eight of them `codex_*` plus `claude`, for every session (`session/state.rs:30-58`).
- About 456 `pub fn` are spread across 95 files in a flat `session/` directory.
- The 400-line cap has produced line-count splits such as `event_{rows,reads,metadata,approval}` and seven `transcript_*` files.

It works, but every daemon command will add more. Add a narrow command/query facade for the session thread and treat the wide surface as legacy. A per-backend state enum can wait.

## 3. Justified, keep

- **Stable ids with offset lookup** (`session/transcript_positions.rs:19-26`): O(1) with no per-row map in the common case. The design supports the flat-latency claim.
- **Bounded journal and borrowed `Published`** (`session/transcript_journal.rs`, `session/events.rs:91-143`): coalesces without cloning text and falls back to reset. It is small, about 350 lines in total.
- **`Change`, `Published::position`, `entry_count` and `dirty_from`**: a legitimate temporary bridge. They should be deleted once the TUI reads a client copy; check that they are.
- **Full-object replacement, decimal-string cursors, separate live cards, opaque ids**: each removes a real client bug class cheaply.
- **`carlos-proto` as serde-only, no tokio, thread per session**: right for fewer than ten clients.
- **Session/view split**: `AppState { session, view }` (`src/app/state.rs:26-29`) is the change everything else needs.

## 4. Simplify or avoid in the remaining EPs

- **Don't commit to UniFFI or WASM for `carlos-client` yet.** Full-replacement events make the phone-side reducer a few hundred lines of Swift. Build `carlos-client` for the TUI and decide on bindings at EP022 with measured app size and build cost.
- **Skip JSON Schema generation** until a code generator actually consumes it; the golden fixture is enough.
- **Defer the 16 ms per-client delta merger.** `publish()` already coalesces per call; add the merger only if a profile demands it.
- **Don't add a second in-process mode for the TUI as a "temporary" measure** without a deletion date. Either the TUI is a client or the embedded path is explicitly permanent.
- **Don't make resets carry more.** The TUI needs the full transcript; solve that with paging or a local-socket bulk read, not a larger `Snapshot`.
- **Keep the SQLite session table to the columns listed**, and don't persist resolved cards or events until the phone design requires it.