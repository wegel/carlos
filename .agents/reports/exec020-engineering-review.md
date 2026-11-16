# EP020 direct engineering self-review

This is Root's direct self-review, not an independent reviewer session. The later Fable review found a healthy-reader defect this first pass missed; the current correction and final checks remain pending. It follows .agents/reviewers/engineering_reviewer.md. Review subject: cc2a5f5..b851d73, runtime source45775d7635fed16302193845c0c4b69b, the completed lifecycle/device/network work and the whole EP016–022 objective. Check evidence: exec020-final-report.md and exec020-final-measurements.json. Final independent Fable review remains required.

I checked the owner stop/final-report/private-result paths, sequence and stale-capability fences, exact Claude fresh/saved targets, owned versus attached subprocess cleanup, transactional catalogue versions, device-file grants and failed revokes, physical Unix versus authenticated device retry rights, operator reserves, upgrade admission/deadlines, socket framing and charged writes. I checked the actual daemon/terminal/native and negative-path evidence against those rules. The listener reuses the one manager/owner and pure client; it adds no generic native JSON entry point or second backend owner.

## Findings

MINOR: one complete terminal run hits the existing500ms helper-stdin deadline on a second automatic Ralph prompt. That run is a real failure. The writer/prompt paths and21 bounded syscall probes do not establish its cause; the complete final gate subsequently passes without a timeout change. Keep its exact traces and the uncertainty in the final report. If it recurs, capture writer-stage/syscall timing rather than extending the deadline or automatically resending. The current failure path closes the transport and reports uncertainty; it does not falsely acknowledge or resend a native prompt.

MINOR: matched terminal cold replay adds a paired1.02ms and resident memory about0.8MiB. The larger/smaller fixtures show a similar fixed resident difference; kernel data do not identify the allocator/code-page cause. Scroll/typing/working/append p95 remain steady at the reported precision. Keep these fixed costs visible; do not create a speculative optimization or claim zero cost.

MINOR: the existing panel read/paging code still repeats in the terminal, as the owner's advisory Fable review found. EP021 must share it in carlos-client and remove matching terminal code before the phone/web consumers. Complete supported lists, reader-close checks and native root/window guards are real behavior, so a truncated panel snapshot is not an equivalent cleanup. New operator commands already use named payloads. Do not treat the provisional wire format as frozen.

## Corrective guidance and verdict

The identified fixture JSON race is repaired at both producers with atomic replacement; strict native reads and original deadlines/assertions remain. No additional blocking code defect was found in this direct review. Keep the three findings above and all failed-run traces visible to the independent final reviewer. The checked runtime and the whole-phone roadmap can proceed if that review accepts the stated risks and required EP021 cleanup.

Verdict: PASS WITH ISSUES.
