# EP021b production source ledger

The baseline is checked tree3ffd3eb, source896435d22d03d85dba4cef0f58ba68e8, closed by4407f6e. Count authored nonblank Rust reachable from production module roots plus helper/src TypeScript; exclude cfg(test)/test-support items, tests, examples, build scripts, docs, generated files and artifacts. Baseline94870 lines: client10263, core46623, proto2420, daemon8318, terminal25593, helper1653.

## Compact tool facts

Current producer event_rows::export_tool hardcodes absent summary/diff counts and guesses a limited kind. The phone's specified tool row needs these existing wire fields; no new phone UI or wire family is justified. Existing pure Core diff/classifier/label code should be reused, with compact facts cached at native updates and no body parsing in a client. Initial test-only change adds a real daemon/native fixture for explicit command exit0, exact Unicode source, one-add/one-delete diff and an unknown command outcome. Production delta is0 at this red-test checkpoint. Record the actual producer/model changes and removed/reused paths after implementation.

The producer now caches optional boxed ToolFacts on the existing Message/EntryDraft and exports them through existing Tool fields. The legacy ToolOutputDetails label record stays unchanged; live and historical generic calls cache their kind without relabeling terminal rows. The shared diff counter also serves the existing terminal label, and the old separate tool_failed parser is removed in favor of one native-outcome reader. Claude preserves its explicit interrupted bit. Fixtures initialize the optional field without inventing native facts. The first checked piece adds219/deletes50 nonblank production lines, net169, allCore:46792 Core lines and95039 total. File counts/deltas are in exec021b-tool-facts-source-count.json. No new crate, client controller, wire family, native owner, model inference or Swift binding is added. Backend metadata checks and later changes will receive separate ledger rows.

## Grok approval choices

Not implemented. Replace process_client's immediate permission_allow_reply and the automatic bypass override; reuse shared cards, native scoped controls and offered option ids. Record native policy/version evidence and production delta when this milestone starts.

## Native history windows

Not implemented. Preserve the full reader for explicit ancestry/rewind, add a checked newest-window/older path where native APIs support it and retain honest compatibility behavior otherwise. The existing Client history merger and bounded source/output APIs stay shared. Record actual backend capabilities, source additions/deletions and native startup cost separately when this milestone starts.
