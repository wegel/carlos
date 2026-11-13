# EP016 part 10.2.2: Claude named messages and recovery

The session now interprets named Claude messages, applies their fences, and owns the failure, switch, clear, capability, prompt and callback rules. `ClaudeDraftContext` borrows composer lines and the local consent token for one synchronous call. `ClaudeUpdate` returns immediate local effects and the existing next backend action. The session never stores a composer, unsent recovery queue, failure badge, terminal widget or mutable client callback.

The reducer borrows the original JSON value. It does not clone an entire history record just to wrap its parameter map. Callback answer payloads still use the existing single-copy command path. Failed submitted text moves once into an explicit recovery report. The app keeps fresh drafts, answer editors, scroll and focus, and the F5 recovery queue. A completed history fragment still passes through the existing outer line dispatcher in the app; moving that last recursive boundary belongs to the next dispatcher step.

The old Claude SDK and control routers were deleted. Client fixture adapters for native callbacks, clear/init, capabilities and session switches call production session commands. The old app item adapter was deleted; its id-less-item test was registered directly under the app so no check disappeared. The app item helper module and other obsolete methods now compile only for client tests.

Checks on 2026-11-13:

- Rust: 1,010 passed, 3 ignored, including two new headless tests. They prove that a failed startup returns submitted drafts once without replacing newer typing, and that a native clear keeps a callback/card alive until its exact cancellation, including an older process epoch. The first test fixtures used generation 0; the actual initial helper generation is 1. The fixtures were corrected before the passing runs.
- Terminal: all 174 workflows passed in 80.501 s. The final two small string cleanups were subsequently checked by the complete Rust suite.
- All 3,000 saved streams, with 51,390 checkpoints, equal parent `597d34c`. No panic. All pre-plan outputs remain byte-identical to part 10.2.1, with the same 103 classified H/T/F differences.
- Release build, format, whitespace and size checks pass. Largest changed file: 354 lines; largest function: 57 lines. The old atomic rename warning remains. No installed binary changed.
- An early strict Clippy check exposed earlier migration cleanup and the Rust 1.100 atomic rename. Its log is `/tmp/carlos-m10c-clippy.log`. The one new unnecessary string copy was fixed. Strict Clippy is still an open final gate; it has not been waived.

Thirty-one alternating samples per endpoint on CPU 6, with the same Rust 1.100/default-feature profile and no competing build/test, gave pre-plan/current synthetic medians: apply 14.13/14.27 ms, total replay 47.15/46.56, layout 14.92/14.66, draw 3.45/3.44, scroll 0.71/0.71 and typing 0.60/0.60. Fixture replay was 1.00/1.01 ms. Counts remain equal. These measurements precede the final unnecessary string cleanups. The cumulative apply gap already recorded in part 10.2.1 remains open for the final gate. Raw samples and saved comparisons are under `/tmp/carlos-m10c/`. Zero RSS fields remain broken and supply no memory evidence.

Direct self-review, applying `.agents/reviewers/engineering_reviewer.md`: reviewed source moves, failure and recovery order, native clear/switch ownership, mixed backend flags, callback consent identity, capability inheritance, history continuation and exact comparison evidence. No BLOCKER or MAJOR finding in this delta. Corrected the unnecessary string copy and retained the displaced id-less-item test. Verdict: PASS for this handler move. The remaining compiler cleanup and cumulative timing gap block final acceptance until handled. This is the primary agent's direct review, not an independent reviewer session; the owner requested no subagents.

The outer native envelope/ready-replay dispatcher, backend action runner, remaining submission/rewind/timer commands and static boundary guard still remain. No binary was installed, merged or pushed.
