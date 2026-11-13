# EP016 part 10.2.1: native callback router

The session routes Codex callbacks and native control replies before root-thread filtering. The app supplies plain panel targets and callback focus, then consumes an immediate report to update local answer editors. The session accepts cards, chooses their focus, checks worker revisions, retires exact callback ids, and routes controls in the original order: task reader, reconnect metadata, partial item reader, goal, panel, interrupt, settings, queue. Settings effects still save defaults before appending the acknowledgment notice. The app applies those effects before queue editor changes.

Two headless tests cover a child callback received during a closed rebuild, wrong-owner resolution, a retired card, form preflight, and a stale form worker. The old app receive/checked helpers remain only as client test adapters. Unused app goal/panel/control reply handlers were removed. No wire field, backend flag rule, or reconnect admission guard changed.

Checks on 2026-11-13:

- Rust: 1,008 passed, 3 ignored. The full suite was run again after the compiler-inlining change.
- Terminal: all 174 workflows passed in 78.082 s. This run preceded the behavior-neutral inlining attribute; final release terminal checks remain due at milestone 11.
- Release build, format, whitespace and file/function limits pass. Largest changed file: 302 lines; largest function: 55 lines. The two existing warnings remain the atomic method renamed by Rust 1.100 and the unused test-only MCP adapter.
- All 3,000 saved streams, with 51,390 checkpoints, equal parent `5569136`; no panic. Pre-plan outputs are byte-identical to part 10.1 and retain the same 103 classified H/T/F differences. These streams do not exhaustively exercise native queue/goal controls; their dedicated Rust and terminal fixtures also pass.

The first timing run showed a larger synthetic apply time. Two follow-ups confirmed a new cost of about 0.2 ms against the parent. Keeping the native callback router out of the shared parser's inlined code removed this new cost. In the final 31 alternating, same-core samples, pre-plan/parent/current synthetic medians were apply 14.07/14.25/14.24 ms, total replay 46.73/46.58/46.53, layout 14.76/14.66/14.57, draw 3.42/3.38/3.42, scroll 0.71/0.70/0.71 and typing 0.60/0.60/0.60. The small fixture replay was 1.02/0.93/1.00 ms. Counts remain equal. All endpoints use Rust 1.100 and default features; CPU 6 is pinned and no build/test overlaps these samples.

The smaller cumulative apply gap against pre-plan remains open for the final speed gate. This report does not waive it. Raw final samples live in `/tmp/carlos-m10b/outlined-perf`; earlier 31-sample controls live in `parent-pairs` and `confirm3`. The original 15-sample summary remains in `perf.log`; its raw sample filenames were reused by the first confirmation run. The parent release was built from the archived parent plus unused observer helpers. The pristine parent archive is also saved for the final compiler comparison. Broken zero RSS fields supply no memory evidence.

Direct self-review, using `.agents/reviewers/engineering_reviewer.md`: PASS on this ownership delta, with no correctness or lifetime finding. Reviewed the exact routing precedence, callback ownership and focus rules, stale worker/card checks, once-only action consumption, settings/queue effect order and closed-rebuild quirk. The inherited cumulative timing gap remains an explicit final acceptance item. This is the primary agent's review, not an independent reviewer session; the owner requested direct work without subagents. No binary was installed, merged or pushed.

Claude routing, the outer Codex envelope/action runner, remaining commands/timers/rewind, the static boundary guard, and final milestone checks remain open.
