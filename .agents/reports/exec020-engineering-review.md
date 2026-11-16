# EP020 final direct engineering self-review

This is Root's direct self-review, not an independent session. It follows .agents/reviewers/engineering_reviewer.md and checks the entire EP020 change plus the phone/web goal. The final runtime source is 6a31fd059ad6f8f0e7e92b1f001bc765; runtime corrections are 3c83655 and 272fae3. The first direct pass missed the healthy-reader defect later found by Fable. Root reproduced that defect with a real socket and repaired it before closure. Both independent Fable reports and all decisions remain alongside this report.

I checked owner stop/final-report/private-result paths, sequence and stale-capability guards, fresh/saved native targets, owned versus attached process cleanup, catalogue upgrades, durable/uncertain device writes, private-file grants, physical Unix/device retry scopes, operator reserves, HTTP/login limits, framing, charged writes and native writer diagnostics. The daemon still has one manager/owner per session and no arbitrary native-JSON command. The client remains pure; neither the server nor client acquires terminal/audio/speech dependencies.

## Findings and corrective guidance

MAJOR, repaired: the original absolute 500 ms whole-frame socket deadline closed a healthy slow reader. The paced 1 MiB test fails before and passes after actual TCP write progress re-arms only the stall deadline. The original stalled/charged write case remains green. The five-second login stays absolute, and bytes stay charged through flush or teardown. Fable's focused follow-up confirms the correction.

MAJOR, recorded risk with better evidence: one earlier complete terminal run hit the native controller's 500 ms stdin deadline on an automatic continuation. Twenty-one bounded syscall probes did not reproduce or explain it. Full gates 03 and 04 pass the workflow. New monotonic queue/guard/trace/write-return stages and trace-off JSON parsing improve future evidence while preserving the deadline, uncertainty and no resend. Keep the original trace and do not claim a root-cause repair. A stage means an attempted write returned, not native acceptance.

MINOR: the final paired cold renderer cost is +1.07 ms and peak RSS about +0.7 MiB; interactive p95 stays steady. The daemon file grows about 2.6 MB for lifecycle, server and operator work. Keep these measured costs visible; do not start a speculative optimization or add a body cap.

MINOR: command classes and terminal panel read/paging rules still need the agreed cleanup before more clients use them. EP021a must share checked read controllers in carlos-client, remove terminal copies, make command admission exhaustive, add a partial-progress-then-stall test and rename the diagnostic written label to returned. Keep complete lists, private editor text and stale-reader/root/window checks. Shared network allowances/identity eviction are documented limits for one operator's devices.

## Evidence and verdict

The complete final release gate 04 exits 0 with 186 actual-daemon, 310 terminal, 23 installed-Codex and 10 packaged-Claude cases, helper 72, architecture/dependency, package, ownership and source/copy checks. Both Rust/lint configurations and the 400/60 source audit pass. Exact frozen-pair performance checks preserve counts and source hashes. See exec020-final-report.md and exec020-final-measurements.json for timing scopes, failed runs and limits. Main, the installed executable and the owner's frozen preview remain unchanged.

No blocking defect remains in this review. Verdict: PASS WITH ISSUES. The agreed EP021a cleanup and recorded native-writer risk remain explicit; the completed lifecycle/network runtime can proceed.
