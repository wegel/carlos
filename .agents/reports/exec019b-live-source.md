# Live original-source reads and exact copying

This report records one EP019b slice on iphone-spike. EP019b remains incomplete. No installed binary, main checkout, merge or remote branch changed.

Carlos keeps the pure Client's8MiB body cache and32window limit. Each original-source window retains a256KiB suffix. One platform worker owns one complete active original and its layout; the terminal retains exact block counts and small painted intervals. The worker receives only bytes already checked by Client. Four requests and one unsent64KiB piece bound transfers; they do not bound the parsed model or the optimal-wrap arrays.

The terminal pulls the initial prefix through existing entry/source reads, one request at a time, then watches its accepted tail. Larger backlogs or actual byte pressure pause the watch and return to pulls. A single pending64KiB quantum waits for its existing watch, so a small announced append does not create another subscription. Failed direct reads close their cache token and stay closed until F6. A replaced daemon retires source jobs and saved row addresses; a normal reconnect preserves accepted bytes. Job and viewport numbers reject stale results even if a replaced cache reuses its token counter.

CtrlY captures the original answer's requested byte prefix and writes it through the worker to the existing clipboard program. The terminal keeps accepting input while that program waits. A selection captures the old body revision/length, layout and row addresses. Existing painted rows and missing body ranges feed the shared SelectionText helper in source order; the worker paints at most64missing body rows at a time. It preserves the old whitespace, Unicode and soft-wrap joins without retaining all rendered rows. Clipboard epochs remain private across selected entries and discard older results. Source failure, removal or a shorter revision cancels a waiting copy and releases its priority. Cancellation also wakes an idle worker so a partial clipboard buffer is released.

## Actual executable checks

The frozen old terminal9b360b3867fe228c9c1e6dfddee4122d846eba4e653e0fdcbaef0e24294da832 rejects both captured64MiB ANSI and16MiB Markdown bodies at the former whole-document cache limit. The same frozen daemon090b40b6dbbcf6c1fecc18a1721a8d25df8e6f96c69d6b6c512416a5487651ec serves old and new terminals. Native frames remain below8MiB; source pieces remain at most65536decoded bytes and the native turns remain unfinished.

Candidate3 SHA3f518451cd7d336a1b8248d42933d2eaa2e65da8da0a802881caa5c567d17cd3 passes both actual large workflows: early/late source at120and28columns, exact legacy ANSI styles, a held live append, private typing, correlated successful F5 restore and full original Markdown CtrlY. The private wl-copy child blocks on a flock before reading stdin; another draft visibly updates before that lock is released. The copied16,777,240bytes match the exact original plus appended bytes by SHA256. The0.675ms blocked-copy draft and~1ms held-source draft values are single functional samples, not percentile measurements. Source/candidate/test hashes, complete logs and rejected setup runs are in /tmp/carlos-ep019b/large-source-pty-handoff.json.

Candidate3 exposed missing selection rows. The real old terminal copies16,860bytes/SHA9ea3552ff33defb4a4661d5855c253a25987302824276aeaa07376573723f44a across inline reasoning, a paged240-line Unicode tool result and an inline answer. After163real wheel views, candidate3 copies only incomplete fragments. Candidate4 SHA3337c318cdea211013fe7b8eeb88ebf4c8e3db3ffcd45492b35687b0d0f96626 matches the exact old bytes in both directions, for automatic mouse-release and CtrlY copies. The committed tests/ui/fixtures/daemon-selection-copy.txt stores those exact synthetic old bytes. scripts/test.sh runs both cases without needing an old executable; the standalone --reference-binary option still compares against a supplied old binary. Handoff: /tmp/carlos-ep019b/selection-pty-handoff.json.

Candidate5 SHA9e9de8412483ef104fcc3268364f07fcb57ca786d6c6c8596a49bfaba24b8ece passes five failure/recovery cases in3.677seconds. Initial and mid-prefix failures stop at requests1and4 through three later numbered appends, then F6 resumes them. Failed original and selection copies release priority so another large visible source loads. A later selected tool receives a much shorter authoritative body and new revision while the earlier clipboard child holds the worker; the selection cancels instead of waiting for its old byte count. Only the already started8192-byte clipboard child runs. Candidate4 fails all five with the same fixture. Handoff: /tmp/carlos-ep019b/source-fail-pty-handoff.json. scripts/test.sh also runs these five cases.

## Gates and measured limits

The first default release gate passed Rust, strict lint, daemon, helper and package checks, then failed4of237terminal cases. Preserve /tmp/carlos-ep019b/live-source-release-gate.log. Two tests assumed the former full-original-byte accounting: their new assertions require bounded suffix retention, one resume for each explicit pause and unique advancing cursors. Two actual small-append watch-churn failures are fixed. All four focused cases pass against candidate5 in7.668seconds: live-source-focused-fixed.log.

The final default release run passes strict Clippy,1367Rust tests/10ignored,61actual-daemon checks,237terminal checks,67helper tests and15architecture checks plus dependency/package checks. Its log is /tmp/carlos-ep019b/live-source-final-release-gate.log, artifacts run-dtQWwE in the isolated checkout. The first newly registered selection case then failed because its inline anchor appeared before the source edge had finished painting. The fixture now waits for the complete requested edge and the opposite edge to leave the screen. Both cases pass once and then five repeated pairs. The same release's remaining5source-failure checks, installed Codex contract and23native-monitor checks pass in live-source-final-remainder.log. No Rust code changed between these runs. Unit checks also cover cached selected rows without a source job, the exact displayed prefix after newer bytes were admitted, and rejecting a shorter revision before waiting for its old length.

Final executables are frozen in /tmp/carlos-ep019b/live-source-final-frozen with a source-hash manifest. The terminal remains candidate5 SHA9e9de8412483ef104fcc3268364f07fcb57ca786d6c6c8596a49bfaba24b8ece; its built daemon is fa94baad2c04886b3390e3395ae1ff7c78e396863900fca752bcfe7d65033c60. The actual64MiBANSI/16MiBMarkdown checks use the same older frozen daemon090b40b6 as the comparison. The first final Markdown rerun exposed a fixture race between reading the first Home rows and resizing before the frame finished. The fixture now also waits for Carlos to restore the composer cursor after painting. All three repeated pairs pass in14.361,14.620and14.659seconds at large-source-final-frame-ack-{0,1,2}; preserve the earlier failure at large-source-final-exact. Both fixture repairs retain exact copy hashes, source/read limits, visible colors, private drafts and reconnect checks. These are completed runtime proofs for this slice, not whole-EP acceptance.

Fifteen alternating default-feature perf-session pairs run onCPU2 with --synthetic --turns1000 --seed1 --tool-lines24 --width120 --height40. Both binaries report6001messages/57651rows/6000relevant messages. Median reported p95 values in milliseconds are scrolling0.65before/0.66after, typing0.37/0.37and append0.41/0.42. Raw pairs and both executable hashes are /tmp/carlos-ep019b/live-source-final-perf/results.json. This ordinary shared-renderer check does not claim a large-source socket speedup or measure the worker's peak memory.

The exact long-line helper retains a measured extreme cost:64MiB dense Unicode can take about6seconds and reach roughly1.3GiB temporary memory while the optimal-fit arrays are built, down from the older upstream~2.1GiB. It retains roughly238MiB after that rebuild. The ordinary captured64MiB ANSI terminal uses roughly78MiB. The worker handles one model at a time and leaves terminal input free, but its arrays grow with word count. No source body cap or queue allowance was increased. Keep the worst-case figures explicit; do not claim constant total worker memory. The completed follow-up reviewer classified this as a MINOR residual risk that does not violate a supplied worker-memory cap and does not block the slice. Whole-EP performance acceptance and the final roadmap review remain open.

## Engineering checks already completed

The initial scoped review returned FAIL for direct-read retries, pending-copy priority and waiting for an older longer revision. Root fixed all three and recorded both positive and negative cases. The follow-up completed before the owner directed that further reviewers wait until the end of EP019b; it returned PASS WITH ISSUES, with only the explicit residual memory risk. No new intermediate reviewer will run. Final engineering and Fable reviews belong at actual EP019b completion.

### Initial verdict

Reviewed the supplied diff, source/copy lifetimes, reconnect paths, and saved evidence. I reran the existing formatter executable: all 10 focused tests passed.

1. **MAJOR: failed direct reads retry automatically.**  
   [source_reads.rs:14](/tmp/carlos-ep019b/formatter-owner-checkout/src/app/remote/source_reads.rs:14) clears `pending` for failed replies without marking the document failed. The Client only closes failed *watch* requests. `drive_reads` therefore issues another direct read on the next tick, including after an initial read failure.  
   **Fix:** latch correlated direct-read failures until explicit retry. Test initial and mid-prefix failures, asserting that request counts stop growing.

2. **MAJOR: a failed source can leave a copy blocking subsequent formatting.**  
   [source_formatter.rs:115](/tmp/carlos-ep019b/formatter-owner-checkout/src/app/remote/source_formatter.rs:115) removes the active worker job when its document token becomes unavailable, but leaves `documents.copy` or `documents.selection` pending. That copy keeps priority over visible entries; its missing token prevents progress indefinitely.  
   **Fix:** cancel the affected copy, release its worker state, and report the source error. Test a failed watch/read during each copy mode, then verify that another large visible entry loads.

3. **MAJOR: a selection can wait forever after its source is replaced by a shorter revision.**  
   [source_formatter.rs:163](/tmp/carlos-ep019b/formatter-owner-checkout/src/app/remote/source_formatter.rs:163) binds the current source revision while retaining the selection’s older source descriptor. In [transcript_formatter_owner.rs:224](/tmp/carlos-ep019b/formatter-owner-checkout/src/app/transcript_formatter_owner.rs:224), the worker waits for the captured byte length before checking the captured revision. A completed replacement shorter than that length can never satisfy the wait.  
   **Fix:** reject mismatched revisions before waiting for bytes, preferably before binding the selection job. Test a selection spanning several entries while a later entry receives a shorter replacement.

The memory evidence shows a remaining production cost: dense 64 MiB Unicode can reach roughly 1.3 GiB temporary memory and take about six seconds to rebuild. One worker and bounded transfer queues do not bound those optimal-wrap arrays. The ordinary capture results support their tested cases; the earlier helper review does not establish production acceptance of this worst case.

**Verdict: FAIL**

### Completed follow-up verdict

Reviewed the original verdict, current fixes, focused test code, and saved logs. The three prior MAJOR findings are resolved:

- Failed direct replies now latch the document and close its token. Tests stop at reads 1 and 4 through three later appends, then recover with F6.
- Failed copies release priority and cancel the worker’s partial selection buffer. Tests confirm another large source loads.
- The terminal and worker reject changed selected revisions before waiting for bytes. The shorter-replacement unit test and multi-entry terminal test pass.

The four earlier gate failures are addressed. The two revised tests correctly check retained 256 KiB suffix windows, balanced watch pauses, and unique resume cursors. The watch code permits one pending 64 KiB chunk while still pausing for larger backlogs or bytes the worker cannot accept. All four focused workflows pass in the saved 7.668-second run; all five failure workflows pass in 3.677 seconds.

I found no new correctness defect in these fixes.

- **MINOR, residual performance risk:** The exact-wrap helper still peaks near 1.3 GiB and takes roughly six seconds on the measured 64 MiB dense-Unicode case. Its arrays grow with word count. This does not violate a supplied worker-memory cap, and the helper improves on the measured upstream peak, so it does **not block this slice**. Keep these measurements explicit in the performance record; bounded transfer queues must not be described as bounding total worker memory.

The committed selection oracle matches the supplied old-output SHA. I inspected focused evidence only; this verdict does not certify the pending full gate or runtime stress reruns.

**Verdict: PASS WITH ISSUES**
