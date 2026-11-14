# EP019b contiguous client source checkpoint

The terminal can request one contiguous original-source string in the existing Bodies cache. Default source and normalized windows still keep 256-KiB suffixes; all 32 windows share one eight-MiB retained-data allowance. A document read never silently drops a prefix. SourceWindow::document returns a borrowed string only after all currently observed bytes arrive. It may return a complete running prefix without claiming native completion.

The cache checks revision, sequence, exact byte positions and cursors before changing data. It computes suffix trims before charging the aggregate allowance. Capacity errors preserve a direct-read cursor for explicit retry; failed initial and live watches close only their read and queue one matching unsubscribe. They never start a backend write, lose private submitted text or repeatedly resubscribe. Replacement watches preserve document mode. String growth doubles geometrically up to the cache ceiling; the allowance counts retained text, not exact allocator capacity or RSS.

The actual-socket Markdown and ANSI captures now replay through this cache rather than an independent candidate String. The formatter borrows the retained source without joining it per draw. All four prior text/style/wrap fingerprints match: Markdown28 f982a61834137336a4a8196ecc65635e1ff624c9d1f4b459da0a8c55b9c91e03; Markdown120 c0bd73715ea8b6851dc30ae785a70d0d9f1e00da2f8b8ef298f3aef9841fcc4f; ANSI28/120 c1f8e250dafa1b868aab64e48c8293f28b6c519658dc38a678ffafa18bc6b4bb. Replay evidence is /tmp/carlos-ep019b/body-document-socket-replay.log, using target/render-probe captures identified in the earlier report. The baseline Message clone exists only in this comparison test. Both sides share the refactored renderer; this is no independent historical parser proof.

No production terminal socket consumer or EP approval is claimed. Eight MiB is a cache bound, not an accepted product body cap. Above-cache fidelity, live source replacement, exact copy/selection and full socket workflows remain required before the embedded path can be deleted.

## Engineering review

Separate ephemeral review against a11d3ed, prompt .agents/reviewers/engineering_reviewer.md. Raw report /tmp/carlos-ep019b/body-document-review.txt:

Reviewed the diff against `a11d3ed`, including all untracked Rust files. Checked shared cache bounds, suffix trims, source completeness, cursor guards, watch replacement/failure paths, and probe borrowing. `cargo test -p carlos-client` passed all 57 tests.

- **MINOR:** [client_body_pressure.rs:23](/var/home/wegel/work/perso/carlos-daemon/crates/carlos-client/src/tests/client_body_pressure.rs:23) tests capacity failure on the first subscription reply, but never after a watch becomes active. That leaves the separate notification failure path untested. Add a test that accepts the first chunk, exhausts the shared allowance, then sends a live update. Assert that the client closes the failed window, sends one matching unsubscribe, ignores late chunks, preserves unrelated windows and submitted text, and never resubscribes automatically. Cover normalized output as well.

I found no blocking correctness defect. I did not run the captured source replay or workspace-wide checks.

**Verdict: PASS WITH ISSUES**

Codex assessment: accepted the MINOR coverage finding. Added a checked live-notification test for both formats: accept the watch, fill another document to the shared limit, append a live chunk, check exactly one unsubscribe, ignore late chunks and preserve the other document/private prompt. The restored 58-test pure client suite and strict workspace Clippy pass. Other tests cover full source borrowing, running Unicode append, duplicate/replaced reads, empty source, local oversized read failure, exact suffix trimming at the aggregate edge, normalized capacity and explicit retry after release. Workspace tests and the real captured-source replay pass. The full terminal gate also passes:49 daemon workflows,183 terminal workflows,67 helper checks and15 architecture checks. Evidence: /tmp/carlos-ep019b/body-document-{tests,client-final,final-clippy,terminal,release}.log. The release build and a 1000-turn synthetic carlos perf-session smoke pass; that still uses the embedded terminal and makes no claim about final socket latency. No binary was installed.
