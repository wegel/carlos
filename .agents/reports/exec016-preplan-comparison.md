# Carlos live-stream comparison before and after ExecPlan 016

This report compares commit `a59221c`, before ExecPlan 016, with `9587748`, after milestone 4. It covers all **24,300 supplied streams**, including Claude native frames. Neither endpoint panicked. The audit found **two regressions under artificial or mismatched input**, two changes required by the plan, and two improvements over old defects. It did not establish an ordinary, unmutated backend path to either regression.

The worker did not edit, build, stash, reset, or commit in `/var/home/wegel/work/perso/carlos-daemon`, and did not access `/var/home/wegel/work/perso/carlos`. It built only detached worktrees under `/tmp/carlos-preplan-*`, each with Rust 1.100.0. Git created and removed the temporary worktree registrations. No proposed fix was implemented.

## Counts and what the numbers mean

The harness compared the transcript and session after **every input event**, then compared the final result, backend calls, panic text, and redraw-call log. A stream counts once in the first three rows below. The later rows overlap those categories.

| Result | Streams |
| --- | ---: |
| Exact match, including every checkpoint and redraw call | **23,099** |
| Only redraw-call logs differ | **867** |
| Session facts differ at some checkpoint | **334** |
| Stored transcript rows differ at some checkpoint | **274** |
| Rendered transcript lines differ at some checkpoint | **260** |
| Any captured screen output differs at some checkpoint | **262** |
| Pre-plan panics | **0** |
| Milestone 4 panics | **0** |

“Rendered transcript lines” includes text, styled segments, roles, separators, cell counts, and wrapping at 160×48 and 60×20. “Any captured screen output” also includes the visible status band and approval-card buffer. Two Claude streams differ only in that additional screen output, which explains 262 versus 260. No stream differs only in rendering while retaining identical session facts throughout.

The final result alone hides some temporary differences:

| Final result differs | Streams |
| --- | ---: |
| Session facts | 305 |
| Stored transcript rows | 246 |
| Rendered transcript lines | 233 |
| Any captured screen output | 235 |
| Recorded backend calls | 8 |

Each backend contributed 8,100 streams:

| Backend | Exact | Redraw only | Session differs | Ever different lines | Final different rows |
| --- | ---: | ---: | ---: | ---: | ---: |
| Codex | 7,571 | 287 | 242 | 233 | 218 |
| Claude | 7,725 | 284 | 91 | 26 | 27 |
| Grok | 7,803 | 296 | 1 | 1 | 1 |
| Total | 23,099 | 867 | 334 | 260 | 246 |

All 300 streams added by the previous fix worker match exactly. The original 24,000 therefore account for all 1,201 differences. The audit also reproduced the earlier result exactly: of its known 86 differing streams, **45 differ only in redraw logs and 41 differ in session results; 38 of those 41 retain different final rows**. All 41 have the generator's artificial `both: true` setting. They belong to class B below.

## Causes and verdicts

The following counts assign each differing stream to one primary cause, so they add to 1,201. H also appears as a secondary difference in 14 session-differing streams: 11 B, one T, one F, and one S. Thus H occurs in **881 streams in total**, of which 867 have no other difference. S and F do not overlap: replaying every differing stream through milestone 3 isolates exactly the 67 F streams.

Verdict **(a)** means the plan requires the change or records it as intended; **(b)** means an unintended regression; **(c)** means an improvement over an old defect that the plan does not record as a screen change.

| Class | Primary streams | Verdict | First commit |
| --- | ---: | --- | --- |
| H: history builder stops logging scratch redraw calls | 867 | **(c)** incidental improvement | `d98a64d` |
| B: Claude cursor guard discards Codex mappings when both flags are set | 241 | **(b)** regression | `d98a64d` |
| S: repeated native Claude assistant source reuses its row | 24 | **(a)** required / recorded | `d98a64d` |
| F: closing Claude also fails an active turn absent from its accepted-prompt set | 67 | **(c)** cleanup improvement | `ac02f0a` |
| P: a disappeared diff part returns as a fresh row | 1 | **(a)** required | `d98a64d` |
| T: foreign-thread history creates keys the active conversation cannot find | 1 | **(b)** regression | `d98a64d` |

No minimal reproducer first changes at `e821bd2`, `2cbfedc`, or `9587748`. The id-less-item panic repaired by `2cbfedc` is not an endpoint difference: both requested endpoints accept the full corpus without a panic.

## H: history builder stops logging scratch redraw calls

**Count:** 867 redraw-only streams: 287 Codex, 284 Claude, 296 Grok. Including secondary occurrences, 881 streams contain this difference.

**Cause and location:** Before the plan, `src/app/item_history.rs::replace_history_from_thread` creates a temporary `AppState`, populates it through ordinary live-row helpers, then transfers its transcript to the real app. The harness's global dirty logger sees calls from both apps. Milestone 2 builds plain history drafts instead. At `9587748`, see `src/app/item_history.rs:34`, `src/app/item_history_draft.rs::build`, and `src/app/state_thread.rs:10`; the latter resets the real render cache once after it adopts the draft.

**Minimal stream: one event.** The oracle supplies the initial thread and normal app defaults. Its old dirty log is `[0,0]`; the new one is `[0]`.

```json
{
  "id": 0,
  "name": "H history redraw",
  "backend": "codex",
  "events": [
    {
      "action": "history",
      "thread": {
        "id": "00000000-0000-4000-8000-000000000001",
        "turns": [
          {
            "id": "t",
            "items": [
              {
                "id": "x",
                "type": "agentMessage",
                "text": "X"
              }
            ]
          }
        ]
      }
    }
  ]
}
```

**What the user sees:** Nothing different. No row, text, fold, order, style, or wrap changes.

**Wider or narrower redraw range, and stale-screen risk:** Neither. The new log has fewer calls, but the effective range is identical. For all 867 redraw-only streams, the audit compared each event's newly added dirty calls, not cumulative lists: every differing event has minimum dirty offset **0 on both sides**. There are no wider or narrower effective ranges. `RenderCacheState::mark_transcript_dirty_from` (`src/app/render_cache_state.rs:41`) keeps the minimum requested offset; the adopted app gets a fresh cache and a full invalidation. Removing calls made on the discarded scratch app cannot leave stale pixels. These counts do not demonstrate a live redraw speed change; they demonstrate fewer scratch helper calls.

**Verdict (c):** The old app built terminal cache state and issued dirty calls while parsing history that it would immediately transfer elsewhere. The new draft builder avoids that unnecessary work. The plan explicitly requires history drafts without public entry ids, but does not prescribe these incidental logger calls, so this report does not label the precise dirty-log change a required user-visible exception.

**Can real backends trigger it?** Yes for a nonempty Codex resume/history replacement; its startup and reconnect paths use this loader. The corpus also calls this common loader directly while the selected backend is Claude or Grok. Those labels alone do not prove their normal native-history paths use the same scratch-app route. The distinction has no visible consequence in any backend.

## B: Claude cursor guard discards Codex mappings when both flags are set

**Count:** 241 Codex streams. All have `both: true`. They have different stored rows at some checkpoint; 217 retain different final rows. Rendered lines differ in 232 streams during replay and 208 at the end. Four streams also send different backend calls after the changed state affects later actions.

**Cause and location:** `AppState::item_key` (`src/app/state_items.rs:13`) correctly chooses Codex when `codex.enabled` is true. However, `prepare_item_source` passes the independent `claude.enabled` flag to `SessionState::prepare_item_source` (`src/app/state_item_sources.rs`, `src/session/item_sources.rs:9`). With no Claude `sourceKey`, `reject_unproven_cursor` (`src/session/item_sources.rs:103`) then unbinds an existing durable **Codex** source just because Claude is also enabled. The next start, delta, or completion cannot find its original row. It appends another one, and may clear the completed guard at the same time.

This explains the earlier report's keyed reasoning completions, assistant deltas, and item starts. They are different callers of the same erroneous guard, not three independent defects. A reasoning completion may duplicate a row; two assistant deltas split one answer; repeated read completions may alter grouping and counts; a repeated start can add a hidden empty shell that later receives text.

**Minimal stream: two events plus `both: true`.** The old app has one assistant row `ab`. The new app has two assistant rows, `a` and `b`. Deleting either event removes this difference.

```json
{
  "id": 1,
  "name": "B mixed backend flags",
  "backend": "codex",
  "both": true,
  "events": [
    {
      "method": "item/agentMessage/delta",
      "params": {
        "turnId": "t",
        "itemId": "x",
        "delta": "a"
      }
    },
    {
      "method": "item/agentMessage/delta",
      "params": {
        "turnId": "t",
        "itemId": "x",
        "delta": "b"
      }
    }
  ]
}
```

**What the user sees:** Split or duplicate assistant/reasoning/tool rows, changed read groups, and sometimes empty hidden rows that later become visible. In longer streams, the changed transcript can also affect later prompt actions. This is a row-addressing defect, not a stale-cache artifact.

**Verdict (b):** The binding rule says a placeholder, its deltas, and its completion keep the same entry. No native item has disappeared or been retired here. A Claude-specific fallback must not remove a valid Codex lookup. None of the plan's three recorded Claude exceptions permits this.

**Can a real backend trigger it?** Not through an ordinary unmutated Codex stream in the inspected app. `configure_codex_app` starts from a new app, enables Codex, and leaves Claude disabled (`src/app/backend_setup_launch.rs:148`); the Claude launch path creates its own app (`:258`). Codex reconnect and rewind do not enable Claude. The generator explicitly writes the incompatible flag combination for selected Codex streams. A corrupt cross-backend fact or a future caller that breaks this invariant could expose it, so it still deserves a fix, but 241 must not be reported as 241 demonstrated real Codex failures.

As a causal check, the audit replayed all 242 Codex session-differing streams with only `both` changed to false. **All 241 B session differences disappear**; the one P difference remains. Eleven of those B cases still have the independent H dirty-log difference.

## S: repeated native Claude assistant source reuses its row

**Count:** 24 Claude streams: 11 first differ on an assistant snapshot and 13 on an empty text-block start. Twenty retain different final rows. Nineteen differ in rendered transcript lines at some checkpoint, sixteen at the end.

**Cause and location:** The old translator uses a generated cursor containing stream/message/block counters. A later message-start or snapshot can give the same native assistant block another cursor. The old app addresses that new cursor as new work. The store now resolves the native assistant message id and block position first: see `src/session/item_sources.rs:9`, `:57`, and `prepare_cursor`, together with Claude's `sourceKey` producer. It binds the new cursor to the original row and retains the completion guard.

**Minimal stream: three native frames.** The old app ends with two `X` rows; the new app ends with one. These are the same native message and block, not merely equal prose. Removing any frame eliminates the difference.

```json
{
  "id": 4,
  "name": "S native source dedup",
  "backend": "claude",
  "events": [
    {
      "native": {
        "v": 1,
        "type": "event",
        "event": "sdk",
        "message": {
          "type": "assistant",
          "message": {
            "id": "m",
            "content": [
              {
                "type": "text",
                "text": "X"
              }
            ],
            "role": "assistant"
          },
          "session_id": "00000000-0000-4000-8000-000000000001"
        }
      }
    },
    {
      "native": {
        "v": 1,
        "type": "event",
        "event": "sdk",
        "message": {
          "type": "stream_event",
          "event": {
            "type": "message_start",
            "message": {
              "id": "m"
            }
          },
          "session_id": "00000000-0000-4000-8000-000000000001"
        }
      }
    },
    {
      "native": {
        "v": 1,
        "type": "event",
        "event": "sdk",
        "message": {
          "type": "assistant",
          "message": {
            "id": "m",
            "content": [
              {
                "type": "text",
                "text": "X"
              }
            ],
            "role": "assistant"
          },
          "session_id": "00000000-0000-4000-8000-000000000001"
        }
      }
    }
  ]
}
```

The other minimal variant replaces the last assistant snapshot with a text `content_block_start` for block 0. The old app adds a hidden empty assistant row; the new app keeps the completed row. That three-frame variant is saved in `source-placeholder-minimal.jsonl`. A subsequent text delta makes the old duplicate visible while the new app ignores it as an update to completed native work.

**What the user sees:** One assistant row instead of a duplicate, or no immediate screen change when only an empty shell disappears. Later duplicates and split text can disappear as a consequence. The app does not merge unrelated equal texts.

**Verdict (a):** The plan requires native Claude assistant id/UUID plus block position to survive changes in generated cursor ids. Its explicit Observation also records reuse of an assistant-text row when Claude sends the same native assistant work again on resume. `src/tests/entry_identity_tests.rs::a_new_claude_cursor_cannot_append_to_completed_native_work` covers the completed-work case. This is not an unrelated deduplication policy introduced by milestone 4.

**Can a real backend trigger it?** Yes for the cause: the plan records repeated assistant work on resume as reachable with real data. The shortest three-frame order above is a manufactured reproducer, not a claimed packet capture. The SDK fixtures contain native assistant ids and block positions; Carlos must use that evidence consistently across live and resumed delivery. Codex and Grok do not produce Claude native assistant keys.

## F: closing Claude also fails an unregistered active turn

**Count:** 67 Claude streams. All 67 differ in session facts during replay; 66 still differ at the end. Seven eventually differ in rows and rendered transcript lines. Nine differ in any captured screen output: the extra two change the later Ralph-mode status-band label. Four send different later backend calls; three draw different approval cards after later common turn events.

**Cause and location:** In the pre-plan app, `ClaudeUiState::fail_pending` fails only UUIDs in its accepted-input queue. A locally recorded active turn that is absent from that queue survives a terminal error. Milestone 3 adds `fail_accepted_prompts`, which also includes the active turn id, clears its boundary, and records failure (`src/app/claude_notifications.rs:213`, `:230`). This is separate from the backend normalizer's normal failure of registered prompt UUIDs.

**Minimal stream: two events.** The mock backend acknowledges the local submit with turn id `owned` but does not emit `claude/promptSubmitted`. After the malformed controller frame, the old app retains `owned: InProgress`, active id `owned`, and its turn boundary. The new app records `owned: Failed` and clears the active id and boundary. Both draw the same error row immediately.

```json
{
  "id": 5,
  "name": "F active turn on fatal error",
  "backend": "claude",
  "events": [
    {
      "action": "submit",
      "text": "X"
    },
    {
      "native": {
        "v": 0
      }
    }
  ]
}
```

**What the user sees:** Usually no immediate transcript change. If a later interrupt targets the orphaned turn, the old app can add a misleading `Turn interrupted` row or send an unnecessary interrupt; the new app does not. Later Ralph toggles can differ because the old app still believes work is active. The ordinary raw status string changes from the transport error to `turn failed`; that string is not itself the visible status band. The retained error row still describes the failure.

**Verdict (c):** A closed Claude query should not leave an orphaned active turn running in app state. F fixes that old cleanup defect; neither the identity rules nor the three recorded Claude exceptions describe it. It is not a reason to restore an orphaned in-progress turn for parity.

**Can a real backend trigger it?** A real helper can fail, but the exact short reproducer is not a complete valid submit trace. The real `ClaudeClient::submit` emits `claude/promptSubmitted` before it calls the helper, whereas the milestone 4 mock only returns `owned` (`src/claude_backend/mod.rs::submit`). Adding that missing prompt fact to the minimal stream makes **both endpoints exactly equal**, including checkpoints and dirty calls. Other corpus cases inject common turn ids not registered as Claude prompts. These 67 therefore demonstrate robustness on inconsistent app/backend state, not 67 failures of ordinary native Claude output. A queued local-input/error race could leave such state, since the event scheduler handles terminal input before deferred server facts, but this audit did not establish that race with a real helper. Resume, reset, and normal ordered registered-prompt failure do not by themselves establish F.

Replaying every one of the 1,201 differing streams through `ac02f0a` identifies exactly these 67 streams and no others. That independently separates F from all milestone 2 source-identity changes.

## P: a disappeared diff part returns as a fresh row

**Count:** One Codex stream, original id **690**. It differs in stored and rendered rows both during replay and at the end.

**Cause and location:** `complete_tool_item` now calls `transcript_retain_parts` before writing its current parts (`src/app/notification_tools.rs:12`). `Transcript::retain_parts` removes the vanished part's aliases (`src/session/transcript.rs:129`, `:141`). It leaves the historical display row, but a later reappearance has no live lookup and receives a fresh row/id. The old index map kept the vanished part's lookup and overwrote its old row when the part returned.

**Minimal stream: three completions of the same item.** It has two parts, then one ordinary read row, then two parts again. The old app ends with two stored rows; the new app ends with three, including the retired `b.rs` diff and its new incarnation. Removing any one event removes the difference.

```json
{
  "id": 2,
  "name": "P disappearing diff part",
  "backend": "codex",
  "events": [
    {
      "method": "item/completed",
      "params": {
        "threadId": "00000000-0000-4000-8000-000000000001",
        "turnId": "t",
        "item": {
          "id": "x",
          "type": "toolCall",
          "name": "Read",
          "input": {
            "file_path": "a.rs"
          },
          "files": [
            {
              "filePath": "a.rs",
              "diff": "@@ -1 +1 @@\n-a\n+b"
            },
            {
              "filePath": "b.rs",
              "diff": "@@ -1 +1 @@\n-a\n+b"
            }
          ]
        }
      }
    },
    {
      "method": "item/completed",
      "params": {
        "threadId": "00000000-0000-4000-8000-000000000001",
        "turnId": "t",
        "item": {
          "id": "x",
          "type": "toolCall",
          "name": "Read",
          "input": {
            "file_path": "a.rs"
          },
          "files": []
        }
      }
    },
    {
      "method": "item/completed",
      "params": {
        "threadId": "00000000-0000-4000-8000-000000000001",
        "turnId": "t",
        "item": {
          "id": "x",
          "type": "toolCall",
          "name": "Read",
          "input": {
            "file_path": "a.rs"
          },
          "files": [
            {
              "filePath": "a.rs",
              "diff": "@@ -1 +1 @@\n-a\n+b"
            },
            {
              "filePath": "b.rs",
              "diff": "@@ -1 +1 @@\n-a\n+b"
            }
          ]
        }
      }
    }
  ]
}
```

**What the user sees:** An added diff row on regrowth instead of an update to the old diff row. Existing rows keep their order; the reintroduced part appears at the tail. The old row is not redirected to the new incarnation of the part.

**Verdict (a):** The binding rules explicitly say an absent display part loses its live lookup, and reintroduced dropped work receives a fresh id. The repository also names this exact exception in `src/tests/entry_lookup_extra_tests.rs::named_exception_absent_diff_parts_retire_and_regrow_with_a_fresh_row`. Restoring the old lookup would violate the plan.

**Can a real backend trigger it?** The exact corpus stream is manufactured: it uses compatibility `toolCall` items with a `files` array and repeats completed work with a changing number of files. The inspected Codex schema uses native `fileChange`/`changes` and `commandExecution` items, not this `toolCall` form. Its patch-update schema permits arrays that can change size, but that alone does not prove this app receives the three completed-item snapshots above; the shared item handler also does not implement the patch-update method as these completions. No inspected unmutated fixture establishes shrink/regrow of one completed item without an intervening authoritative history replacement. Claude/Grok result adapters can supply diff text, but their ordinary fixture paths likewise do not establish the exact three-snapshot sequence. Treat P as a deliberately supported identity edge case, with ordinary-backend reachability unproven, rather than inventing a protocol ordering guarantee from a JSON schema.

## T: foreign-thread history creates unreachable live keys

**Count:** One Grok stream, original id **1208**. It differs in final and intermediate stored rows and rendered lines. It also contains H.

**Cause and location:** `replace_history_from_thread` detects a response for another thread and rewrites draft keys to that response's conversation (`src/app/item_history.rs:34`, especially its `NewConversation` branch). It then adopts the rows but does not change the app's current thread/entry conversation. A subsequent valid live key still belongs to the active conversation and cannot resolve the adopted row. The old raw-id map did resolve it. In the corpus, a Grok rewind first changes the active thread to `fork-thread`; a later direct history action supplies the original thread again, then Grok updates a tool id in that history.

**Minimal stream: two events.** The initial app belongs to `00000000-0000-4000-8000-000000000001`. The history belongs to `other`. The old app updates its one read row to `b.rs`; the new app leaves the foreign `a.rs` row and appends the live `b.rs` row. Deleting the history removes T; deleting the update leaves only the independent H logger difference.

```json
{
  "id": 3,
  "name": "T other-thread history",
  "backend": "grok",
  "events": [
    {
      "action": "history",
      "thread": {
        "id": "other",
        "turns": [
          {
            "id": "t",
            "items": [
              {
                "id": "x",
                "type": "toolCall",
                "name": "Read",
                "input": {
                  "file_path": "a.rs"
                }
              }
            ]
          }
        ]
      }
    },
    {
      "grok": {
        "sessionUpdate": "tool_call",
        "toolCallId": "x",
        "title": "read_file",
        "kind": "read",
        "rawInput": {
          "path": "b.rs"
        }
      }
    }
  ]
}
```

**What the user sees:** An old history row remains, and a second row appears instead of updating it. With a longer foreign history, later live events can add further duplicates. This is a mismatch between the adopted keys and the app's current conversation, not a fold or renderer defect.

**Verdict (b):** Conversation-scoped keys are required, but no binding rule requires accepting a foreign history into an app whose current conversation was not adopted. A valid switch must adopt the thread and its facts together; an invalid response should leave the prior session intact. The new helper can instead create two owners that disagree about the same displayed rows. Merely calling the old cross-conversation overwrite an improvement would miss that inconsistent state.

**Can a real backend trigger it?** The demonstrated Grok route is an artificial common-history action, not normal Grok rewind. The production Grok fork helper resets the app to the returned thread before adopting its copied, unkeyed prefix (`src/app/turn_submit_history.rs:38`). The Codex fork helper similarly changes the thread before it adopts keyed history (`:60`); reconnect code fences the requested thread and connection. Claude has its own explicit session-change path. None of those ordinary paths produces this minimal mismatched-thread state. A corrupt/stale response accepted through the generic loader, or a future caller that skips the explicit switch step, can produce it. Its current practical reachability is lower than a normal backend-stream defect.

## Commit checks with the minimal streams

The worker built the real app at each named commit with the same oracle and replayed every minimal stream. “Old” means identical to `a59221c` for that reproducer; “new” means the class has appeared. The saved output includes full snapshots, not just this table.

| Class | `e821bd2` | `ac02f0a` | `d98a64d` | `2cbfedc` | `9587748` |
| --- | --- | --- | --- | --- | --- |
| H | old | old | new | new | new |
| B | old | old | new | new | new |
| S | old | old | new | new | new |
| F | old | new | new | new | new |
| P | old | old | new | new | new |
| T | old | old | new | new | new |

The worker also deleted each event from each minimal stream in turn. Every deletion removes the corresponding session difference; the one-event H reproducer becomes empty. T with its live update deleted still has H, which is expected and is recorded separately. These are event-minimal, structurally simple reproducers, not merely truncated original cases.

## Regressions ranked by user impact, with proposed fixes

Both regressions rank below a demonstrated ordinary-backend regression. The counts are counts in a mutator, not estimates of how often a user encounters them.

1. **B, split or duplicated live content when backend flags disagree.** Fix `src/session/item_sources.rs::prepare_item_source` / `reject_unproven_cursor` so the Claude cursor guard checks the actual typed backend/native-key kind, rather than trusting the independent Claude-enabled flag. Codex and Grok native keys must retain their lookups; preserve the existing tests that reject unproven Claude cursors, and add the two-delta mixed-flag reproducer. This is first because one bad flag can corrupt every subsequent live update, although ordinary startup does not set that flag combination.

2. **T, foreign history adopted without adopting its conversation.** Make the common history loader validate the expected conversation before it changes any rows or keys, and reject a mismatched response while retaining the old session. Keep intentional fork/switch callers on their explicit path that parses first and then adopts the new thread and its keyed draft together. This is second because the current production callers already take those explicit paths, and the demonstrated Grok trigger bypasses them.

Do not undo S or P to regain old output; those changes implement the binding rules. Do not restore the scratch invalidations in H or the orphaned active turn in F. If the project wants exact compatibility even for the F mock's inconsistent state, it should first decide that explicitly and improve the mock to model real prompt acknowledgements.

## Method, source checks, and practical limits

The worker copied `target/m4/streams.jsonl` and the augmented `target/m4-fix/streams.jsonl`, verified that the first 24,000 JSON values agree, and preserved all 300 added cases. It reused the supplied generator, fixtures, mutation seed `0x164004`, and Rust replay loop. Running the original generator again produced the same **24,000 JSON values** with zero structural differences; byte hashes differ because the supplied stream file uses a different JSON serialization. The full replay used the supplied corpus, not a replacement generated from a different seed.

For Codex, the loop sends app-server messages through the actual app event dispatcher. For Grok, it first calls the production `translate_grok_notification_for_test`, then dispatches those messages. For pre-plan Claude, the adapter recovered from the milestone 3 audit runs `ConversationGuard` and envelopes accepted native frames as `claude/controller`; the old app's own handlers translate them. For modern Claude, the existing milestone 4 adapter runs the production reader normalizer, history decoder, and `ClaudeClient::handle_event` before app dispatch. Both adapters use deterministic test stream counters; native ids and text remain unchanged. The old mock's `interrupt` adapter is a no-op because the old app itself captures the interrupt in its Claude lifecycle state.

The canonical output follows the prior pre-plan reduced oracle: it removes the Carlos row id, approval entry id, and the typed-key-to-Carlos-id diagnostic map, none of which has the same representation before the plan. It retains every row's role, kind, text, path, and tool facts; turn statuses, active turn, boundary, approvals, Ralph state, context, status, thread/name/phase; backend calls; redraw calls; rendering; panic text; and a hash after each event. Additional per-component hashes and optional full checkpoint dumps identify which event and which part changed. The removed id/key diagnostics are not treated as screen differences. The exact reproduction of the known 86-case comparison checks compatibility with that earlier reduced oracle.

The worker read local protocol code, the vendored `docs/app-server-schema/` JSON, the generator inputs, the plan's binding identity section, its Observation about three Claude cases, and the milestone 3 parity report. `schema-evidence.json` extracts required thread/turn/item fields and the native Codex item variants. A JSON schema describes field shapes; it does not prove that a backend repeats completed items or emits a particular event order. The report separates actual app paths, plan-recorded real-data behavior, synthetic corpus settings, and unproven orders accordingly.

The audit covers the supplied synchronous event/action order. Like the milestone 4 oracle, it does not append an extra EOF event to truncated streams, launch real backend accounts, or enumerate reader/UI thread schedules. It renders at two widths after each event; width changes can force full cache rebuilds, so that alone would not prove absence of stale-cache bugs. The separate dirty-range audit establishes that the **867 redraw-only** cases request the identical effective range at each differing event and reset the adopted cache, which is the relevant proof here. No wider or narrower effective dirty range was found among those cases.

All 24 shard processes exited successfully and produced exactly 24,300 results per endpoint. Tests ran with `--no-default-features`; the audit does not claim to test dictation. The worker applied only temporary test adapters, read-only snapshot helpers, visibility changes needed by those tests, and the dirty-call hook in detached trees. It saved those patches before removing the worktrees. All six investigation worktrees and their Rust overrides have been removed; `cleanup.json` records each successful removal.

## Files a fix worker can rerun

The full evidence directory is `/tmp/carlos-preplan-artifacts/`:

- `m4/streams.jsonl`, `m4-fix/streams.jsonl`, `added-300.jsonl`, generator sources, copied fixtures, `generator-verification.json`, and `sha256.json` identify the exact inputs.
- `audit-{old,new,m1,m3,m2,fix}.rs`, `native-{old,new}.rs`, `ralph-helper.rs`, `install-harness.py`, and `harness-*.patch` preserve the test code and temporary source changes.
- `bin-{old,new,m1,m3,m2,fix}` are the six runnable Rust test binaries. They do not need the removed worktrees.
- `old-0.jsonl` through `old-11.jsonl`, and their `new-*` counterparts, contain every full-corpus result, component hash, and dirty log. `runs.json` and shard logs record exit status and time.
- `summary.json`, `differences.json`, and **`class-manifest.json`** enumerate all 1,201 differing stream ids and their primary causes. `class-counts.json`, `render-counts.json`, and `history-dirty-inclusive.json` supply the counts used above.
- `session-{old,new}.jsonl` contain full event-by-event facts and renders for all 334 session-differing streams. `single-backend-*` isolates the mixed-backend cause. `dirty-range-audit.json` verifies effective redraw bounds for every redraw-only case.
- `minimal.jsonl`, `minimal-{old,new}.jsonl`, `bisect-{m1,m3,m2,fix}.jsonl`, `bisect-results.json`, and `minimal-deletion-checks.json` preserve all six minimal streams and the commit checks.
- `registered-failure-*` shows F disappearing when the mock supplies the real prompt-acknowledgement fact. `m3-cause-audit.json` independently identifies the 67 F cases across all differing streams.
- `source-evidence/` preserves the cited source files from `9587748`; the plan and milestone 2/3 reports are also copied alongside this report. Source line numbers above refer to the committed files, before test hooks were added.

To rerun the saved binaries without rebuilding:

```bash
rtk proxy python3 /tmp/carlos-preplan-artifacts/run-corpus.py
rtk proxy python3 /tmp/carlos-preplan-artifacts/compare.py
rtk proxy python3 /tmp/carlos-preplan-artifacts/replay.py /tmp/carlos-preplan-artifacts/minimal.jsonl minimal
```

To rebuild endpoints from the named commits, `rebuild.py` creates fresh detached `/tmp/carlos-preplan-rebuild-*` trees, sets Rust 1.100.0 in each, installs the saved test hooks, builds with separate target directories under the artifact directory, saves the binaries, unsets the temporary overrides, and removes its worktrees:

```bash
rtk proxy python3 /tmp/carlos-preplan-artifacts/rebuild.py old new
# Optional intermediate commits:
rtk proxy python3 /tmp/carlos-preplan-artifacts/rebuild.py m1 m3 m2 fix
```

`replay.py INPUT TAG` writes full old/new checkpoint dumps. Its Python `replay(label, cases, tag, trace)` helper also accepts the intermediate labels for repeating the commit checks. Neither rerun command edits the shared working tree. The saved original generator and historical installer scripts are evidence; the supported self-contained installer is `install-harness.py`.


## Outcome (integrator, 2026-11-12)

- B and T are fixed on `daemon-split`. B: the Claude cursor guard reads the backend kind of the item. T: the common history loader rejects a history whose thread id differs from the active thread, before it changes anything.
- T is a deliberate difference from the pre-plan code, not a restoration: the pre-plan code adopted a foreign history and still resolved later updates by raw id; the new code keeps the current session and returns `History belongs to another thread`. No production caller can reach it (each one builds the app from the id that the history carries, and Codex reconnect rejects a foreign snapshot earlier). Consequence in the mutation corpus: stream 1208 and three sampled exact-match streams (2360, 22991, 23082) now keep the fork's rows where both older endpoints cleared them, and stream 285 logs one redraw call less.
- H, F, S and P stay as they are.
- Replay after the fix: all 241 B streams match the pre-plan session facts, rows, screens and backend calls; S, P and F are unchanged; 1,997 of 2,000 sampled exact-match streams stay exact (the other three are the T consequence above). Proof files: `/tmp/carlos-bt-proof/` (not kept in the repository).
