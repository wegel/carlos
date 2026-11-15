# EP019b direct engineering self-review

Reviewer: root, directly. This is a self-review, not an independent reviewer claim. Independent Fable evidence is recorded separately. Subject: complete EP019b relative to accepted EP019a, separate removal commit cc2a5f5, final review corrections after 0421d92.

Root inspected the owner report/closure/queue/ordinary-command paths, client reconnect and operation recovery, terminal retry/disconnect behavior, native reconnect receipt, deletion inventory, dependency checks, test-owner map and final performance evidence. Automated checks cover the remaining workflow surface; this review does not claim a line-by-line manual read of every moved file.

The owner retains queued recovery under the exact ClientOperation before native work and settles it privately after closure. Removing the old terminal report does not expose a draft to another device. Client reconnect queries outcomes and restores reads; it does not replay mutations. The client has no socket, timer, thread, terminal or native process. Default terminal startup attaches the managed daemon; no embedded session or backend remains in the executable path.

## Findings and corrective guidance

MAJOR: shared Core diagnostics still named F5/restart Carlos after the terminal was removed. Root removed those instructions and kept platform-neutral backend facts. The terminal retains local key hints.

MINOR: report fields and reconnect arguments still carried obsolete editor/focus effects. Root removed their producers and consumers and kept private operation acceptance/fork facts. Existing native order tests retain their protocol assertions; the retired-prompt test now checks the actual rejection status.

MAJOR: the closed non-Codex F5 path could wait indefinitely for a helper it never reopened. Root added a truthful local hint without a new subscription, process or send. The strengthened actual-terminal case fails the previous release pair. The explicit stop/reopen command and 16-owner-slot recovery belong to EP020's now-required first milestone before network access.

MINOR: panels recognized disconnect text by string prefix. Root replaced that test with an explicit Device disconnect reason and reused the existing disconnect/reconnect workflows.

The measured source0c interactive path is faster than both references; its full cold replay and admission have small real fixed costs. The dense Unicode index and two unattributed historical timeouts remain explicit limits. None should be described as fixed by ordinary-corpus results. Refreshed corrected-source measurements remain required for final acceptance.

## Verdict

PASS on the inspected code after the listed corrections. Final corrected-source feature/release/UI/native checks and measurements are still pending; this verdict does not approve the EP before those checks finish. Fable's complete PASS WITH ISSUES findings and root's dispositions remain in exec019b-review-disposition.md.
