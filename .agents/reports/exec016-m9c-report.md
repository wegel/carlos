# Milestone 9.3.1: native metadata and legacy history

The session now owns Codex enabled/closed flags, the native epoch, the shared request counter, and temporary item turn/part fields. App code reads a copied scope and uses named commands. Codex and Claude flags deliberately remain independent: generated fixtures exercise both flags together. Enabling Codex still changes only that flag; it neither reopens a closed connection nor disables Claude. The old independent index-map reference remains unchanged.

The session builds backend entry keys with the original backend priority, conversation, turn filter, native prompt classification and display part. Summary code borrows the resulting owned key instead of borrowing the session while mutating it. No transcript or item is cloned to avoid that borrow. Temporary item turn/part values still restore their exact prior values.

The legacy Codex history selector and rollout parser moved unchanged into session/codex. The backend snapshot reader calls them directly, and its app history export is gone. Local file bounds, eligibility markers, fallbacks, item order checks and failure returns are unchanged. Tests import the new owner directly.

All 997 Rust tests and 174 terminal workflows pass (79.346 seconds). The release build, format, whitespace, file-size and function-size checks pass; the largest changed file is 398 lines and the largest function 60. The existing atomic API rename warning remains for final toolchain cleanup.

All 3,000 parent streams match at 51,390 checkpoints with no panic. Pre-plan output matches part 9.2 byte for byte, including its 103 classified differences. This corpus does not cover exhaustive native reconnect schedules; existing reconnect fixtures cover those. Seven quiet alternating performance pairs show no repeatable slowdown. Artifacts are under /tmp/carlos-m9c.

The primary agent reviewed the moved guards, defaults, scope restoration and history parse paths against 3891d1a using the engineering reviewer prompt. Findings: none. Verdict: PASS. No subagent or independent reviewer ran.
