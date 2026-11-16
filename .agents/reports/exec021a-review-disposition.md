# EP021a review decisions

Fable5.2 medium reviewed the whole phone/web goal and representative shared-reader/daemon/terminal code at9cc99ff. It returned PASS WITH ISSUES, no BLOCKER or code defect, one MAJOR planning concern and six MINOR findings. Session40ac7fb9-6f52-4398-885c-a37ed169cc3e cost$2.6465074. Its verbatim report and metadata remain in exec021a-fable-final-review.md/.json. It did not read the architecture document, full ExecPlan or raw measurements/diff outside the worktree, and did not verify the core's private native-context wrappers. Those limits remain explicit; Root's direct review and actual checks cover those paths.

## MAJOR: bind the first phone screens, not every Rust panel

Accept the concrete concern. PROGRAM_PLAN.md and docs/engine-architecture.md now require EP022's UniFFI facade to follow the actual first-screen engine needs: greeting/reconnect, session list/lifecycle, transcript/history/source/output, prompt/interrupt/approval, context display and Ralph. Shared queue, host context-default, goal and native task panels stay Rust-only until a phone screen needs them. The terminal uses those readers today, so they are not unused prototype code, but EP021a's benefit to the first phone is largely indirect. No automatic mirror of all177 public methods, speculative sheets or whole-panel Swift records is authorized by their Rust visibility. A later web binding follows the same rule.

Do not impose an arbitrary hard line quota that could trade correctness or complete data for a headline. Instead EP021b must record added/deleted production source per native feature, identify removed paths and explain net growth against the phone's actual needs. Keep the existing400/60 source-shape limits. Root treats the binding boundary as resolved for this plan and carries the source ledger into the next plan; no code rollback or extra framework is justified by this finding.

## MINOR1: Effects must be acknowledged

Fix now with #[must_use] on Effects. Strict lint initially identifies the previously implicit returns. Inspect every production call: point-cache closes/cancel return local model notices only; the greeting probe has no session watches; the operator's failed write returns local uncertainty while its connection exits. Make those acknowledgements explicit and explain the two transport exit/probe cases. Fixture-only feeds also acknowledge Effects explicitly, without suppressing the lint. Calls that can close native goal/task/window/activity interest continue passing outgoing effects to the platform transport. This is a compiler guard, not a new effect framework.

## MINOR2: selected detail target

Fix now. TaskPanel::selected_detail_target builds window/revision/task from the accepted selected row. The real Rust consumer and terminal both use it, eliminating their two manual triples and the terminal's invariant expect. The helper grants metadata context, not native control consent. It remains Rust-only unless a later actual screen needs it.

## MINOR3: exhaustive reply routing and manual reader hooks

Make the ReadReply router exhaustive now. Explicit unowned devices, prompts, saved pages, task-run, source and output variants still pass through Effects.replies; every future reply variant must choose a route at compile time. Keep the small explicit cancel/disconnect/byte-charge lists instead of a macro, trait or registry. When a later feature adds reader state, its tests must prove those existing hooks; no new reader family is needed merely to satisfy symmetry.

## MINOR4: phone cache policy

Defer to the actual EP022 facade. The current checked64MiB policy deliberately detaches the affected session, keeps other sessions/private submissions and closes native interest through Unsubscribe. The three large-input cache test already proves that behavior. A constructor-supplied phone limit and typed UI notice should be chosen with its real memory/call site; neither is a demonstrated current defect. This follow-up is recorded in the architecture note rather than adding unused policy knobs now.

## MINOR5: invariant expects behind future bindings

The reviewer traced the listed expects and found none reachable from invalid replies. Keep existing validators and scope checks. EP022 must check the chosen UniFFI panic/error contract and exercise malformed/out-of-order input against the actual exported facade. Do not add a speculative panic wrapper or broad randomized framework to these readers without a concrete call site or reachable failure.

## MINOR6: monotonic operation counters

Confirmed. connection.rs:115-124 keeps max(local, remote) for a resumed lease and preserves exhaustion; next_operation:153-169 advances with checked_add. carlosd retry/existing.rs:30 rejects a new operation at or below the lease's high-water mark; an existing id only retrieves the original canonical request/result. clients.rs:138 greets with high_water+1, or exhaustion. A replacement lease gets a different private client id. The existing exhausted_counter_preserves_reads_and_never_wraps_to_one test covers wrap refusal. Add the confirmed invariant beside the owner batch sort; no allocator change is needed.

## Final state

The direct review is PASS WITH ISSUES and the independent final review is PASS WITH ISSUES. The minor source corrections above need final source-matched checks and measurements before delegated approval. The checked9cc99ff pair and every prior failed trace remain frozen, and target/preview-9cc99ff/try-carlos provides that fully checked CLI without installation or a backend launch. No further reviewer is required for these narrow, inspected corrections unless a new substantive defect appears.
