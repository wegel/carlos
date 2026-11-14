# EP019b large-source baseline

The source/history slice is committed as95d6a73. EP019b remains incomplete. This experiment measures the existing formatter before choosing above-cache storage or a different row cache. It does not approve a product body cap, add a production large-source path or close the terminal cutover gate.

## What ran

`scripts/probe-large-source.py --binary target/release/carlosd --artifacts target/large-source-probe` creates private native fixtures and sends ANSI output or Markdown through the actual daemon. Native frames stay at or below64KiB and original-source replies at or below64KiB. The turn stays open after the fixture sends all known body bytes. These are fully received current bodies in a running turn, not completed native item records. A completed64MiB command item would exceed the native8MiB frame allowance; running tool source also omits the final command header/footer. Initial smoke checks incorrectly expected that final wrapper/Ok status and were fixed before measuring.

The script checks every returned UTF8-safe source offset and hashes every original byte. It records the entry, descriptor, session and chunks. The daemon's memory samples remain separate from the Python fixture process. Exact capture identities follow:

- Daemon: `090b40b6dbbcf6c1fecc18a1721a8d25df8e6f96c69d6b6c512416a5487651ec`, `/var/home/wegel/work/perso/carlos-daemon/target/release/carlosd`.
- ansi: 67,108,864 bytes in1024 reads; SHA256 `1845226d523271fc15e979d320742aa0f2d29d558c6be8eb350bce39bcbbdf9d`; socket reads 550.52ms.
- markdown: 16,777,216 bytes in257 reads; SHA256 `f7b91e7d09fe28d6b65a17bab87c67cdf2b467c08a76c21a0b076c0f0975b578`; socket reads 201.91ms.

To reproduce formatter runs, build the test executable with `cargo test --release --no-default-features --bin carlos --no-run --message-format=json > target/large-probe-build.jsonl`, then run `python3 scripts/probe-large-format.py target/large-source-probe --build-json target/large-probe-build.jsonl --samples 16`. The runner creates private clipboard program shims and config homes, pins CPU 6, records the executable hash/profile and verifies every copied source digest. The original baseline used the equivalent scratch runner before this helper was saved in the repository.

The ignored Rust `large_source_layout_probe` first replays each captured chunk through the existing bounded suffix validator. Only accepted chunks enter the test-only pinned String. The suffix validator stays bounded and is dropped before source memory is sampled; the production eight-MiB document allowance remains unchanged. `RenderInput::from_client` still checks the complete descriptor and bytes. The probe invokes the actual BlockBody count/materialize functions, verifies row counts and visible tail text, and fingerprints row text/styles/wrap flags. This measures the formatter offline, not socket or whole-UI latency.

The Markdown corpus is mostly one large literal code fence, with one reference definition and one link outside it. Its numbers do not establish parser cost for documents with many separate Markdown elements.

Each case/width runs in its own process on CPU6. The isolated release test executable comes from a detached95d6a73 snapshot with only the probe files added. It uses `--no-default-features` and its own Cargo target directory to avoid racing the independent goal-panel worker. The formatter has no dictation feature branch. Its hash/profile is distinct from the normal product release and must not be presented as an end-to-end release cutover measurement.

- Formatter executable: `ee3f9e5eadc68e34d1794f920d62e34a0be92b5937f9247d7dc5cc61a46cf835`, `/tmp/carlos-ep019b/large-probe-target/release/build/carlos/d88de9bc036dd20d/out/carlos-d88de9bc036dd20d`.

The probe uses a private clipboard shim with the real clipboard writer and verifies the resulting file against the capture SHA256. Thus even above8MiB, the pinned original can supply exact copied bytes. This is a prototype consumer; the actual terminal Ctrl+Y above the production cache still needs its storage/lease path.

For16 synthetic append samples the probe updates the pinned current body and descriptor, then retires the old raster, counts and materializes again. For16 resize samples it alternates28/120columns. Timings include source append and dropping the prior raster, but exclude the diagnostic capacity scan and output fingerprint. The total also includes one read/parse of /proc/self/status after counting; the separate count/materialize fields exclude it. This small unmeasured diagnostic cost cannot explain the multi-second totals, but this probe should not establish submillisecond differences. The count/materialize split is recorded separately. VmRSS/VmHWM are sampled inside the executable, without inherited Python high water. Summed Vec/String capacities count retained row storage but exclude allocator headers. RSS includes mapped code, temporary parser buffers and allocator-retained pages, so it is not a precise heap partition.

## Results

- ansi, 28 columns: 3,195,660 rows; count 1289.13ms, materialize 2417.20ms. Pinned-source RSS 73,508KiB; first-raster RSS 803,300KiB; later process high water 809,348KiB. Retained raster capacities 619,958,300bytes. Append p95 7118.06ms; resize p95 7706.84ms. Exact original copy verified; clipboard write 63.80ms.
- ansi, 120 columns: 798,915 rows; count 274.04ms, materialize 234.05ms. Pinned-source RSS 73,544KiB; first-raster RSS 431,640KiB; later process high water 808,252KiB. Retained raster capacities 327,555,154bytes. Append p95 516.61ms; resize p95 3702.82ms. Exact original copy verified; clipboard write 72.17ms.
- markdown, 28 columns: 629,146 rows; count 543.81ms, materialize 900.39ms. Pinned-source RSS 24,440KiB; first-raster RSS 183,940KiB; later process high water 212,640KiB. Retained raster capacities 149,736,674bytes. Append p95 1324.19ms; resize p95 1208.29ms. Exact original copy verified; clipboard write 32.81ms.
- markdown, 120 columns: 209,717 rows; count 80.24ms, materialize 77.66ms. Pinned-source RSS 24,404KiB; first-raster RSS 130,884KiB; later process high water 213,468KiB. Retained raster capacities 83,466,884bytes. Append p95 132.34ms; resize p95 1180.82ms. Exact original copy verified; clipboard write 31.30ms.

Raw metrics, individual samples, original clipboard bytes and row/style digests are under `target/large-source-probe/{ansi,markdown}/width-{28,120}.{json,log}` and clipboard-{width}.txt. Summary output is `/tmp/carlos-ep019b/large-probe-render.log`. The separate one-MiB smoke cases passed at both widths before these measurements.

## Decision and next work

A file or read-only mapping alone would retain the dominant full-raster allocation and full-document reformat cost. Do not add a file worker or mmap path now. Keep one original source and make the formatter retain only the rows needed around the viewport, with a small index that can locate earlier rows. Reuse the current wrapping and style rules and prove exact text/style/wrap parity against them. ANSI/plain/diff can use source positions and saved color state to revisit a bounded region; Markdown must still parse a whole document because reference definitions can occur elsewhere. Measure that parsed-document memory separately from owned painted rows. Keep expensive work outside the input/paint loop, using a bounded formatter job only if incremental work cannot stay within a frame.

The existing bounded suffix validator already proves exact chunks while an external pinned String accumulates them. Inspect whether the platform can consume those accepted pieces directly before adding a new pure-client pass-through API. Do not duplicate validation, add a generic scheduler or raise the eight-MiB cache limit. If a narrow source-ownership transfer or data-only pass-through is actually needed, add it with an actual caller and token/revision/cursor tests. Keep the actual large-source screen, append, resize, copy, memory and process tests open until that production caller works.

The normal baseline perf-session/source-cache checks from the prior slice remain in `.agents/reports/exec019b-sparse-source-history.md`. All production terminal parity, final engineering/Fable reviews and the separate embedded-path deletion still belong to EP019b.


## Engineering review

Reviewed the specified probe code, test hook, helper visibility change, and report. The four code files match the isolated `95d6a73` checkout. I independently verified captured offsets, descriptors, source and clipboard SHA256 hashes, and all four sets of 16 append/16 resize samples.

No BLOCKER or MAJOR findings.

- **MINOR:** [transcript_large_probe.rs:141](/var/home/wegel/work/perso/carlos-daemon/src/app/transcript_large_probe.rs:141) calls `memory()` inside the `total_ms` interval. The total therefore includes reading and parsing `/proc/self/status`. Subtract that diagnostic duration or document it explicitly. This overhead does not undermine the observed multi-second formatter cost.

- **MINOR:** [probe-large-source.py:123](/var/home/wegel/work/perso/carlos-daemon/scripts/probe-large-source.py:123) places almost the entire Markdown fixture inside one code fence. Add that caveat to the report: these numbers describe mostly literal code text, so they do not establish parser costs for Markdown with many separate elements. This does not undermine the evidence that retaining every formatted row is expensive.

The evidence supports the next step of retaining rows around the viewport. The report appropriately leaves production storage, whole-document Markdown memory, and actual terminal behavior open.

**Verdict: PASS WITH ISSUES**

Both MINOR findings are recorded explicitly above. The measurements retain their original values and artifact identity; no diagnostic duration has been invented or subtracted afterward. The review independently verified offsets, descriptors, original/copy digests and all sample counts. This accepts the baseline evidence for choosing the next formatter change, not production above-cache support or EP completion.
