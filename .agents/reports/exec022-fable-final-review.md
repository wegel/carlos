# EP022 final independent review

Claude Fable 5.2, medium effort, read-only snapshot at `90634a7e` with final checked gate results. Review session `6437dac6-d8f1-4f4e-ab2e-71a438c7b341`. Reviewer output follows.

## Summary

EP022 holds the architecture it set out to prove. Swift has no wire decoder, cursor, history merger or reply-rights logic: `PhoneTransport` only moves opaque frames, and every command is rechecked in Rust against the current connection and snapshot. Stale sockets are fenced by generation, body and question handles are opaque and owner-checked, and all reads are bounded. The roughly 7.6k added lines are proportionate, and shared client, core, protocol and daemon growth is small and targeted.

The web path is clean at the daemon and `carlos-client` level, but not at the bridge level: a fair share of `carlos-phone` is platform-neutral policy that a WASM binding cannot reuse as written. That is a first step for the web plan, not an EP022 defect.

I found one lifecycle defect worth fixing before closing EP022.

## Findings

**MAJOR 1 — Failed and uncertain operations are never released (fix in EP022)**

- **Where:** `apps/ios/Sources/PhoneHostModel.swift` `accept(_:)` holds the only `forgetOperation` call, and it runs only for `.accepted where !needsEcho`.
- **What leaks:** every rejected or uncertain send, fork, approval, interrupt, rename, Ralph toggle or reopen stays in `operations`/`operationOrder` and in the Rust store for the life of the process.
- **Consequence 1:** `crates/carlos-client/src/operations.rs` `begin` refuses all new work at 128 records, or about 2 MiB of retained text because of the 2× reservation. The user then gets "private client operations are full; … handle existing drafts first" with no control that handles them.
- **Consequence 2:** `PhoneRecoveryView` offers Restore and Copy but no dismissal, so "Submitted text retained" cards accumulate in the transcript, even after the text is restored and re-sent.
- **Not verified:** a prompt accepted just before switching sessions may keep `needs_echo` true indefinitely once its session is unsubscribed. Worth checking in the same fix.
- **Guidance:**
  - Forget text-less control operations as soon as their error is surfaced.
  - Forget rename and fork-edit records when their editor closes.
  - Add an explicit dismiss to the recovery card, and forget on Restore, since the text then lives in the draft.
  - Add one simulator case that fails more than 128 controls and then sends.

**MINOR 2 — Per-update work is linear in the loaded window (measure, then decide)**

- **Where:**
  - `PhoneHostModel.received` builds `Set(orderedEntries)` and `Set(rows.keys)` on the main thread for every session batch, including each streaming delta.
  - `PhoneHostUpdate.order` re-reads every entry id over FFI whenever a new row appears.
  - `PhoneTranscriptCellItem.read` and `PhoneTranscriptController.update` rebuild the whole item list and dictionary on every content change.
- **Why it matters:** the only cap is the shared client's 100,000 rows / 32 MiB (`rows.rs`), and the real replay evidence in the report is a 76-row capture.
- **Guidance:** `PhoneWindowChange` already carries `removed_front`/`removed_back`, so appends and prepends can be applied incrementally. Measure a window of several thousand rows first; this is hardening, not an EP022 blocker.

**MINOR 3 — Neutral rules live in the UniFFI crate (do first in the web plan)**

- **Where:**
  - `approval_inputs.rs` (`checked_fields`, `encode_field`, `selected_values`) and `form_answers.rs` encode answers.
  - The terminal has its own encoder in `src/app/remote/questions_answers.rs`, and it also validates the form schema, which the phone leaves to the daemon.
  - The guards in `approvals.rs::approve`, `controls.rs::interrupt`, `bodies.rs::open_body` and `transcript.rs::older_history` are likewise platform-neutral.
- **Why it matters:** a browser binding would need a third copy of the answer encoding.
- **Guidance:** move these functions, which need only `carlos-proto`, `carlos-client` and `serde_json`, behind neutral types in `carlos-client`. `carlos-phone` and the web binding then stay thin type adapters. Do not do this ahead of the web consumer.

**MINOR 4 — Second Markdown mapping, and it sits in the shared client**

- **Where:** `crates/carlos-client/src/markdown` is used only by `carlos-phone`; the terminal keeps its own pulldown mapping in `src/app/markdown_tables.rs`.
- **Cost:** `carlosd` now depends on a crate that pulls in a parser it never calls.
- **Guidance:** acceptable as the future shared structure for web. Either have the terminal adopt it later or feature-gate it; do not let a third mapping appear.

**MINOR 5 — No liveness check on an established socket (mobile hardening)**

- **Where:** `PhoneTransport` has a 15 s greeting deadline but no ping afterwards.
- **Why it matters:** a half-open connection after a network change reads as connected until a write fails. Forced reconnect after backgrounding (`CarlosApp.resumeConnections`) covers the common case.
- **Guidance:** a periodic `sendPing` with a deadline is enough, later. Explicit Reconnect instead of automatic retry is a tested preference and I am not reopening it.

**MINOR 6 — `PhoneHostModel` is the place that will grow**

- **Where:** about 30 mutable fields cover link, catalogue, row cache, operations, approvals, editors and body models. It also locally overrides Rust-owned `currentSnapshot`/`ready` in `changedLink` and `PhoneHistoryModel.latest`.
- **Why it is safe today:** the override is display-only, because Rust rechecks every call.
- **Guidance:** split operations/echo tracking from the transcript cache before the next feature lands there.

## Scope limitation

This was a static read; I ran nothing. I read `carlos-phone` production code nearly in full, the central Swift models, transport and transcript controller, and the `carlos-client` and `carlosd` hunks of the patch. I did not read:

- most SwiftUI view files or any test bodies;
- the `carlos-core`, `carlos-proto` and terminal text-wrap hunks;
- `docs/engine-architecture.md` beyond its web references;
- most of the ExecPlan's Progress log.

I did not verify the initial snapshot size, which bounds how soon finding 2 matters.

## Verdict

**PASS WITH ISSUES** — fix MAJOR 1 under EP022; the rest are recorded decisions for the web plan and later mobile hardening.

