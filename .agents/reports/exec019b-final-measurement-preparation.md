# Final formatter measurement preparation

One ignored test now drives the actual formatter thread through the pure Client's Bodies cache. It admits at most 64 KiB per source piece and retains only the cache suffix on the calling thread. The existing worker owns the full original. The test adds no runtime request, wire field, worker type or public API.

The test records the accepted source length, cache-data high water, actual retained row count and row payload capacities. It reads Linux kernel RSS/peak RSS before source admission, after admission, after the first index and after samples. Row payload figures exclude spare slots in row vectors and allocator overhead; the kernel figures include both. The worker's complete-source digest must match the streaming input digest after all appends.

Each source runs separately. Sixteen append samples and sixteen samples in each resize direction wait for the count/head frame and the final tail at 28 and 120 columns. Admission finishes before the first layout. These figures measure worker/cache behavior, including local handoff; they exclude socket/terminal painting and the clipboard program. Exact final production-pair workflows prove those separately. The pure-cache limit is unchanged and does not bound the worker's parsed model or optimal-wrap arrays.

## Reproduce after the final source is frozen

Build the default-feature release test artifact with cargo test --release -p carlos --no-run --message-format=json. Record the executable hash, compiler/profile/features and source hash. On a quiet host, pin the test to CPU6 and run only formatter_worker_source_metrics with --ignored --nocapture --test-threads=1. Set CARLOS_FORMATTER_SOURCE_PATH, CARLOS_FORMATTER_CASE (ansi or markdown), CARLOS_FORMATTER_RESULTS and a unique CARLOS_FORMATTER_RUN_ID. Reject missing/stale result identities or mismatched input sizes/digests.

Use the verified 64 MiB ANSI and 16 MiB Markdown captures listed in /tmp/carlos-ep019b/final-gate-preparation/perf-inputs.json. That manifest also identifies the pre-EP016 and EP019a perf-session references and EP019a daemon for the real Unix-socket admission driver. Use kernel peak RSS for the old perf-session command's known zero-RSS display; do not change that pre-existing counter in this EP.

## Preparation evidence and open work

Small ANSI 348000-byte and Markdown 168000-byte checks pass on the private deletion preview, with fresh result identities, exact full-source digests, sixteen samples per direction and at most 48 retained rows. Strict frontend all-target Clippy passes there. Those runs used a busy host and dev profile, so their timing values are excluded from final evidence. The code is a measurement tool; no performance improvement is claimed by adding it.

The exact source capsule is /tmp/carlos-ep019b/worker-probe-proof.json. Final release measurement, actual above-cache source/copy/selection workflows, matched perf-session and daemon admissions remain open. No reviewer or EP approval has run.

The whole-function audit also found one existing callback test at66lines. Its eight-line field setup moves unchanged into a small test helper; the original five-case table and all answer assertions remain. The test is now59lines, below the repository limit.

Both small source checks also pass after registration in the primary tree: /tmp/carlos-ep019b/worker-probe-primary-smoke/{ansi,markdown}.log. Fresh identities, source lengths/digests, sixteen samples in each direction and48row high water are checked; all dev timing values remain excluded. The unchanged five-case Codex field-answer test passes after its helper extraction, and the broad cap check reports399lines/file and60lines/function at most. Strict no-default workspace/all-target Clippy passes.
