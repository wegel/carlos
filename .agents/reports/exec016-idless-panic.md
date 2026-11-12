# Carlos tool-completion panic investigation

I investigated commit `0f5f2b4` and replayed the same small streams in detached worktrees. I did not edit, stash, reset, commit, build in, or install from the shared checkout. All source references below refer to `0f5f2b4` unless I name another commit. The uncommitted `notification_tools.rs` still has the same failing expression at line 45.

## 1. Exact panic and smallest streams

`src/app/notification_tools.rs:45:27` panics while evaluating `item["id"]` in:

```rust
if let Some(id) = item["id"].as_str() {
```

Here `item` is a `&serde_json::Map<String, Value>`, obtained with `as_object()` at line 32. The map delegates `Index` to its backing map, which panics when the key is absent. The code never reaches `as_str()` or `retain_parts()`. This is a missing JSON field, not a missing native-item-to-transcript mapping. By contrast, indexing a `serde_json::Value` returns `Null` for an absent field.

Starting with an empty `AppState`, the entire smallest stream I found is one line:

```json
{"method":"item/completed","params":{"item":{"type":"toolCall"}}}
```

No start, delta, turn, thread ID, prior transcript row, rewind, or duplicate is needed. Removing the method, params, item object, or recognized type prevents this call. A normal JSON-RPC envelope and matching `threadId`/`turnId` do not prevent the panic.

The same one-event stream panics with Codex, Claude, or Grok selected **if injected at the common app-handler boundary**. This does not mean each native backend can emit that stream:

- Codex forwards app-server JSON to the common handler, so a malformed app-server event can reach this line unchanged.
- Claude constructs common items from native messages and always adds an ID. I found no native Claude stream that produces an item without that field.
- Grok constructs common items from ACP messages and either adds an ID or skips the message. I found no native Grok stream that produces an item without that field.

Replacing `toolCall` with any recognized tool kind also reaches line 45: `tool_call`, `toolInvocation`, `functionCall`, `mcpToolCall`, `toolResult`, `toolOutput`, `tool_result`, `functionCallOutput`, `mcpToolResult`, or `commandExecution` (`src/app/tools.rs:18,25,32`).

Controls, replayed across all three backend selections:

- Absent `id`: panic at HEAD.
- `"id":null` or `"id":123`: no panic. The field exists, and `as_str()` returns `None`.
- `"id":"unknown"`: no panic; the handler creates a mapped row.
- Complete `x`, complete `x` again, then start `x`: no panic; one tool-output row remains.

I also checked every recorded panic against the exact next event after its last successful snapshot in `target/m4/head.jsonl` and `streams.jsonl`. All 1,244 were Codex-labelled streams stopped at an `item/completed` with no `item.id`: 594 `commandExecution`, 338 `toolCall`, and 312 `toolOutput`. No recorded panic needs reordering as its explanation. Stream 12, event 11 (zero-based), is a representative `Read` tool call with its ID deleted. No Claude-labelled or Grok-labelled stream accounts for these 1,244 failures.

## 2. Commit that introduced the crash

I built and replayed the identical one-event stream through `handle_server_message_line`, with each backend selected, at all five requested commits:

- `a59221c`, before ExecPlan 016: no panic, no row for the bare minimal item.
- `e821bd2`, milestone 1: no panic, no row.
- `ac02f0a`, milestone 3: no panic, no row.
- `d98a64d`, milestone 2: panic, `no entry found for key`, line 45.
- `0f5f2b4`: the same panic at line 45.

ExecPlan 016 introduced this crash in `d98a64dfd35048e738a393f710f9137f48e1c033`, `session: give transcript entries stable ids`. That commit added:

```rust
if let Some(id) = item["id"].as_str() {
    app.messages
        .retain_parts(&app.item_key(id), diffs.len().max(1));
}
```

The new block retires mappings for diff parts that a later completed item no longer contains. Its unchecked JSON-map read introduced the crash. The parent code went directly from collecting diffs to the no-diff/diff completion helpers, both of which already tolerate an absent ID. The later uncommitted move from `app.messages.retain_parts` to `app.session.transcript_retain_parts` preserves this read.

## 3. Could a real backend send it?

A backend must omit the ID field to cause this particular panic. Merely changing event order or the transcript cannot remove that field.

- **Codex:** The checked-in schema requires a string `id` for all 19 `ThreadItem` variants. In particular, see `docs/app-server-schema/codex_app_server_protocol.schemas.json:27035` for `commandExecution` and the `ThreadItem` variants in the same definition. A completion for an unknown, completed, or retired ID still contains an ID and cannot cause this read to panic. However, Carlos parses Codex messages as `Value` and forwards method-bearing JSON without checking each item's schema (`src/protocol_reader.rs:81,103`). A buggy backend, incompatible producer, or corrupted fixture that omits the field can therefore crash Carlos. I found no evidence that the real Codex backend legitimately omits it. The generic `toolCall` reproducer is a Carlos compatibility kind; `commandExecution` gives the same crash using a current native Codex item kind.
- **Claude:** Streamed tool calls choose a native ID or generate one (`src/claude_backend/translate.rs:177`), then explicitly serialize `id` at line 313. Assistant snapshots do the same (`snapshot.rs:128,156`). Both normal tool-result constructors insert an ID (`tool_results.rs:87,109`). A result with no known tool call takes `fallback_tool_result_item` (`user_record.rs:31`), which emits `"{tool_use_id}:result"` with an explicit ID (`tool_results.rs:154`). Even a native result missing `tool_use_id` becomes `":result"`, not an item with no ID. Claude history uses the same translator (`event_history.rs:102`). After a reset, the reader clears its tool-call state (`event_live.rs:33`), so a surviving unknown result can take that fallback; it still has an ID. The reader and app also reject older conversation epochs, the counters that distinguish a reset conversation, and retired process generations (`event_normalizer.rs:55`; `app/claude_notifications.rs:27`). Existing fixtures cover retired-helper events (`src/tests/claude_backend_tests/local_history_recovery.rs:26`).
- **Grok:** `translate_tool_call` requires a nonempty `toolCallId`, otherwise it returns (`src/grok_backend/translate_tools.rs:18`). It explicitly serializes that ID at line 45. `translate_tool_update` likewise rejects a missing ID at lines 62–69; a completed unknown tool ID is allowed and emits `"{tool_id}-result"` at line 93. The translator does not require a preceding call to make the result safe.

I replayed these native controls at HEAD:

```json
{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"unknown","content":"hello"}]}}
```

Claude emits one common `toolResult` with `id:"unknown:result"` and appends a row. Remove `tool_use_id`, and it emits `id:":result"`, also without panic.

```json
{"method":"session/update","params":{"update":{"sessionUpdate":"tool_call_update","toolCallId":"unknown","status":"completed","rawOutput":"hello"}}}
```

Grok emits one common `toolResult` with `id:"unknown-result"` and appends a row. Remove `toolCallId`, and it emits nothing.

I replayed a Claude reset followed by an unknown native result through `FixtureNormalizer`, which uses the production reader normalizer and event handler. It emitted a keyed `late:result` item and appended a row without panic. I also replayed a common keyed completion after each of: rewind to row zero, replacement with empty history, explicit cursor retirement, and reset. All three backend selections completed without panic and kept the transcript maps consistent. Retirement can leave the old display row and create a new keyed row; rewind/history/reset can remove the old row and create a replacement. A Claude completion carrying an older epoch was ignored.

These checks establish that the proposed reorder/reset schedules do not explain this panic. They do not prove that every backend schedule has correct display semantics. A real Codex process could still emit a buggy frame, but that frame would violate the checked-in protocol.

## 4. Previous behavior a fix must preserve

At `a59221c`, `e821bd2`, and `ac02f0a`, the no-diff helper reads the optional ID with `get("id").and_then(Value::as_str)` and handles items without IDs separately. See `a59221c:src/app/notification_tools.rs:57,87,94` and its diff helper at line 106.

The following are each complete, one-event streams. Wrap each item in `{"method":"item/completed","params":{"item":ITEM}}`:

- `{"type":"toolCall"}`: ignore; append no row.
- `{"type":"toolResult","output":"hello"}`: append one unkeyed `ToolOutput` plain row containing `hello`, with tool details.
- `{"type":"toolCall","name":"Read","input":{"file_path":"same.rs"}}`: append one unkeyed `ToolCall` row containing `→ Read same.rs` in an empty transcript. The existing read-summary helper may combine its visible text with a preceding read row in a nonempty transcript.
- `{"type":"commandExecution","command":"pwd","aggregatedOutput":"/tmp"}`: append one unkeyed `ToolOutput` row containing `$ pwd\n/tmp`.
- `{"type":"toolResult","diff":"@@ -1 +1 @@\n-a\n+b\n"}`: append one unkeyed diff row, with tool details. Multiple diff blocks append multiple rows.

I replayed these examples on all three pre-crash commits and all three backend selections. HEAD panics on all five because they omit `id`. A fix should make the new part-retirement block conditional on `item.get("id").and_then(Value::as_str)` and then allow the existing fallback helpers to run. Returning immediately whenever `id` is missing would lose the previous output/name/diff rows.

## 5. Audit of other map, vector, unwrap and expect sites

I inspected every requested file at `0f5f2b4`, including `item_history_draft.rs` and `item_history_predecessors.rs`. I also followed the child module `state_item_sources.rs` and the callers of the transcript's unchecked setters. I found **one reachable event-driven map/vector panic in this scope: `notification_tools.rs:45`**. The judgments below concern these accesses, not every possible arithmetic, allocation, or formatting panic in the program.

The store keeps `positions`, `entries`, and `active` together when it appends, truncates, resets, or adopts history. Backend strings never become vector offsets directly. The app resolves a backend key to a live `EntryId`, then to its current position. Callers apply one event synchronously, so another event cannot replace the transcript between that lookup and the access. `put_item_mapping` gets an index just appended or checked by its caller. These facts support the safe judgments; the store tests also check them explicitly (`src/session/transcript_invariants.rs:8`).

### `notification_items.rs`

- Lines 21, 28, 105: `item["id"]` / `v["id"]`. **Safe:** these receivers are `Value`, not `Map`; absent fields and wrong JSON shapes yield `Null`.
- Line 169: `item_message_index(id).expect("native item row")`. **Safe:** `ensure_item_placeholder` immediately before it either finds a live row or appends and binds one.
- Line 181: the same expect for a tool-output start. **Safe:** same preceding guarantee.
- Line 200: the same expect after a user-message upsert. **Safe:** the upsert has just bound the row; this branch runs only when it inserted one.
- Line 242: the same expect for completed user input. **Safe:** same upsert guarantee. An unknown native ID creates a row; it does not leave the lookup empty.
- No bare `.unwrap()` sites.

### `notification_tools.rs`

- Line 45: `item["id"]`. **Reachable panic:** the one-event stream in section 1. This receiver is a `Map`.
- Line 85: `item_message_index(id).expect("native item row")`. **Safe:** `upsert_mapped_message` immediately before it updates a live row or appends and binds one.
- Line 122: `item_message_index(key).expect("native item row")`. **Safe:** the same upsert runs inside the same `with_item_part` closure, so both calls use the same part number. The handler does not retire mappings between these two calls.
- No bare `.unwrap()` sites.

### `notification_turns.rs`

- No direct map/vector/slice subscripts, bare `.unwrap()`, or `.expect()` sites. The parser uses checked object and field access.

### `state_items.rs` and its source-key child

- Line 62: `self.messages[idx].id` in `activate`. **Safe for app events:** callers pass a newly appended index, a checked last-row index, or a checked history-prefix index. `ensure_item_placeholder` and `upsert_mapped_message` do not delete the row they just appended.
- Line 64: `self.messages[idx].id` in `bind_item`. **Safe:** the same index, with no intervening transcript change.
- Line 140: `self.messages[idx].text`. **Safe:** line 136 has just resolved this live item mapping.
- Line 141: `self.messages[idx]`. **Safe:** same checked lookup and no intervening change.
- Child `state_item_sources.rs:130`: `self.messages.entry(id).expect("live cursor")`. **Safe:** `id` came from the store's live item map; truncation and history replacement remove stale aliases. A missing source key may unbind a cursor, but it does not introduce a dangling ID.
- No bare `.unwrap()` sites.

### `tool_output_items.rs`

- Line 17: `item["id"]`. **Safe:** the receiver is `&Value`; an absent ID yields `Null`.
- Line 103: `item["aggregatedOutput"] = ...`. **Safe:** the receiver is a mutable `Value` cloned from an object with `type:"commandExecution"`; `IndexMut` inserts the absent field. `command_output_missing` cannot be true for a scalar, array, or null.
- No bare `.unwrap()` or `.expect()` sites.

### `item_history.rs`

- Line 66: `turn["id"]`. **Safe:** `turn` is a `Value`; absent fields yield `Null`. This enclosing append-whole-history helper is test-only at HEAD.
- Line 109: `item["id"]`. **Safe:** `item` is a `&Value`.
- Line 113: `item_message_index(key).expect("native history row")`. **Safe:** the immediately preceding upsert binds the row, inside the same part closure.
- No bare `.unwrap()` sites.

### `item_history_draft.rs`: JSON values

Each following subscript is **safe** because the receiver is `Value`, and reads of missing fields yield `Null`:

- Line 57: `thread["turns"]`.
- Line 58: `turn["id"]`.
- Line 59: `turn["status"]`.
- Line 60: `turn["id"]`.
- Line 63: `turn["items"]`.
- Line 107: `item["id"]`.
- Line 140: `item["id"]`.
- Line 154: `record.item["type"]`.
- Line 157: `record.item["id"]`.
- Line 203: `item["id"]`.
- Line 209: `item["type"]`.
- Line 214: `item["text"]`.
- Line 279: `item["text"]`.

### `item_history_draft.rs`: vector/slice positions

The draft's `positions` map stores only offsets allocated by `upsert_row`; the draft never removes entries while it builds. Backend IDs select among those stored positions rather than supplying an offset.

- Line 120: `entries[index].draft`. **Safe:** `index` came from `upsert_row` at line 118.
- Line 126: `entries[index].draft.file_path`. **Safe:** same valid index.
- Line 131: `entries[previous].draft.clone()`. **Safe:** `previous` comes from `coalesce`, which returns a predecessor within this same vector.
- Line 135: `entries[index].draft.clone()`. **Safe:** same `upsert_row` index.
- Line 160: `entries[index].draft.kind`. **Safe:** `record.native_index` came from the draft's positions map at line 142.
- Line 165: `entries[index].draft`. **Safe:** same valid native index.
- Lines 183 and 184: `entries[index].occurrences` and `entries[index].draft`. **Safe:** the map contains only indices recorded when this grow-only vector appended a row at lines 187–195. Duplicate keys reuse those live positions.
- Line 304: `entries[index].draft`. **Safe:** `coalesce` receives the result of `upsert_row`.
- Line 308: `entries[previous].draft`. **Safe:** `index > 0` was checked at line 301. The predecessor is below `index`, or the fallback is zero, which exists because `index` exists and is greater than zero.
- Lines 312, 313, 314, 315: `entries[previous]` / `entries[index]` edits. **Safe:** the same two checked positions; the function clears content but removes no rows.
- No bare `.unwrap()` or `.expect()` sites. `unwrap_or(0)` is a checked fallback, not an unwrap that panics.

### `item_history_predecessors.rs`

- Lines 39, 42, 44, 46: `words[word]`. **Safe:** lines 36–37 resize the vector to include `word` before each access. The function derives the bit location from an existing history-row index.
- Line 75: `self.words[level]`. **Safe:** line 65 stops recursion when `level == LEVELS`; callers start at zero and increase by one. The inner word read uses `.get(word)`.
- Line 80: `self.words[level][previous_word]`. **Safe:** the same level guard applies. The recursive upper-level bitset can return only a word that `set` already allocated at the lower level. Clearing a bit updates that upper-level marker when the lower word becomes empty.
- No bare `.unwrap()` or `.expect()` sites.

### `src/session/` production code

- `transcript.rs:104`: `self.active[index]`. **Safe:** `position(id)` returned a live position; the store appends/truncates the active flags with the entries.
- `transcript.rs:111`: `self.active[index]`. **Safe:** both item-to-ID and ID-to-position lookups succeeded first.
- `transcript.rs:117`: `self.active[index]`. **Safe:** guarded by `position(id).is_some_and(...)`.
- `transcript.rs:124`: `self.active[*index]` and `self.entries[*index]`. **Safe:** the function generates indices from `start..entries.len()`. If `start` exceeds the length, the range is empty; both vectors have the same length.
- `transcript.rs:237`: `self.entries[index].file_path`. **Safe for app events:** both production callers pass a just-appended row (`state_items.rs:118`; `notification_tools.rs:92`). This API would panic if arbitrary internal code supplied an invalid offset, but no backend field is used as that offset.
- `transcript.rs:242`: `self.entries[index].kind`. **Safe for app events:** production callers obtain the index from a live item lookup, an appended/checked prompt row, or the grow-only history draft. See `state_input.rs:142`, `claude_prompts.rs:64,99`, `claude_session_history.rs:99`, and `state_thread.rs:67`.
- `transcript.rs:247`: `self.entries[index].tool_output`. **Safe for app events:** the only production caller is `describe_tool_output`, which checks `messages.get(idx)` first (`tool_output_items.rs:14`). Its intervening code does not remove entries.
- `transcript.rs:258`: `self.entries[index].tool_output.as_mut()?`. **Safe:** `index` came from `position(id)?` on the preceding line.
- `entry_id.rs:27`: `counter.checked_add(1).expect("entry id counter exhausted")`. **Safe against event-supplied keys/indices:** the counter is private, starts at zero, and is not parsed from backend IDs. Exhausting it would require 2^64 allocations in one namespace; this explicit resource limit is not an unknown-item crash.
- Additional non-subscript panic checks: `transcript.rs:192` uses `split_off(position)` after `truncate_before` resolves a live position; `transcript.rs:169` and `transcript_aliases.rs:10` assert that internally supplied IDs are live. **Safe for these event paths:** callers use just-appended, retained, or resolved live IDs.
- `history_identity.rs`, `entry_key.rs`, `models.rs`, and the remainder of `transcript_aliases.rs` have no unchecked event-key subscripts or bare unwrap/expect sites. They use `get`, `get_mut`, `entry`, `remove`, and optional values.

### Test-only sites

The following cannot be reached by a running Carlos backend; they use local test fixtures or assert internal invariants:

- `session_headless_tests.rs:19,22`: expects immediately after inserting a local row.
- `transcript_identity_tests.rs:57,58,59,60,61,152,168`: subscripts on saved fixture ID arrays; line 59 also expects a fixture row that the test deliberately retained.
- `transcript_invariants.rs:51`: `pop().expect("new row")` immediately after a test helper appends it; line 58 indexes the insertion position supplied by that test helper.
- `entry_id_tests.rs` deliberately drives the private counter to its exhaustion boundary under `#[should_panic]`.

I retained the literal search inventory under `/tmp/carlos-panic-index-inventory.txt` so a reviewer can check the scope and distinguish executable subscripts from attributes, type declarations, and literal arrays.

## Evidence and limits

I used Rust 1.100.0 in each detached worktree and `cargo test --no-default-features panic_probe -- --nocapture`. The tests call the real app handlers and catch each expected panic to print both the error and remaining rows. HEAD also runs separate native-translator and lifecycle probes. The verified logs contain a fresh compile for each commit. I forced each crate to recompile before the verified run because a shared target directory can otherwise reuse a newer executable from another worktree whose sources have older timestamps. Earlier logs without `-verified` are exploratory and should not be used as evidence for the commit matrix.

- `/tmp/carlos-panic-a59221c-verified.log`
- `/tmp/carlos-panic-e821bd2-verified.log`
- `/tmp/carlos-panic-ac02f0a-verified.log`
- `/tmp/carlos-panic-d98a64d-verified.log`
- `/tmp/carlos-panic-0f5f2b4-verified.log`
- `/tmp/carlos-panic-verified-results.json`: checked commit matrix and compile provenance.
- `/tmp/carlos-panic-probes.rs`: common replay cases.
- `/tmp/carlos-panic-native-probes.rs`: HEAD native and lifecycle cases.
- `/tmp/carlos-panic-corpus-summary.json`: all 1,244 corpus triggers, grouped by backend and item type.

I did not call a live model or rely on a real user session. The backend verdicts come from the checked-in protocol code, schema, fixtures, and deterministic replays. I made no production fix.

I removed all five investigation worktrees, their Rust directory overrides, and the temporary shared build directory after saving the probes and verified logs. I left pre-existing worktrees alone.
