# EP019b: saved rows for general ANSI and plain lines

The earlier ANSI prototype could draw ordinary output and one ASCII word quickly,
but it reparsed and walked the prefix of a general long line for every viewport.
The new formatter saves each long line's wrapped row ranges and style-consumer positions.
The formatter can then draw a requested row without visiting its preceding rows.
The new plain-message owner uses the same source/tab map and complete-line wrapper.
All these modules still compile only under the test-only ANSI prototype hook.
Root owns the production source owner, formatter thread and Markdown caller.

The final release measurements and bounded read-only engineering review are complete.
No Fable review, EP approval, archive, installation or merge belongs to this slice.

## What the formatter retains

- Both page owners retain one original string through its existing `Arc<String>` handle and expose that text
  by borrow. `into_original(self)` moves that same handle into another formatter model.
  The tests check its pointer and sole owner; neither getter clones the full source.
- Ordinary ANSI batches keep the existing owned parser and save source/row/style positions at physical line breaks.
  They do not allocate original-range metadata for every parsed span.
- An oversized batch uses the same upstream ANSI parser through its already-enabled
  borrowed `to_text()` API. The formatter subtracts the generated style prefix from
  borrowed span pointers and checks every range against the unseeded original bytes.
  It never searches for visible text inside escape codes or adds another ANSI lexer.
- The source map keeps one 16-byte record for each original span and one 16-byte
  record for each expanded tab. It stores no second original or expanded text string.
  A caller can provide `text_with` to copy requested original ranges from separately
  retained strings. The accessor test drops the temporary flat line before reading
  across separate styled spans, including a grapheme that crosses their boundary.
- Each saved ANSI row holds its displayed byte range, the exact byte range consumed
  by the old style rule, and its soft-wrap flag. A plain row needs its displayed range,
  blank/code kind and wrap flag. The owners release unused vector slots after indexing
  and drop the old long index before append or resize creates another one.
- Ordinary checkpoints carry a pointer to a rare long index. They do not carry all
  its vector headers in every checkpoint. The metadata counter includes each boxed
  index and every retained vector slot separately from the original source.

The formatter still asks textwrap to choose breaks for the complete logical line
with its existing default algorithm and options. It preserves CJK, combining marks,
emoji joins, tabs, trailing spaces, CR/LF and ANSI styles. It preserves the existing
default-style fallback when the old style consumer's text differs from a wrapped row.
It imposes no body or line byte cap and does not replace the wrapper with a greedy one.

For lines above 16 KiB, the helper feeds compact records to textwrap's public
`wrap_optimal_fit` function. It uses the same public word separator, `split_words`
iterator, cached word widths and default penalties as the existing `wrap()` call.
Each normal record keeps a full-size source end and checked 32-bit width/space counts
in 16 bytes, compared with upstream `Word`'s 56 bytes. If an individual field does
not fit, the helper converts the existing records and all later records to 24-byte
records with full-size fields. It neither truncates those values nor rejects a word
or body. A focused test exercises both exceptional fields without allocating a huge
string. Ordinary bounded inputs keep their existing `wrap()` path. The helper also
uses that path for an embedded newline or a changed default algorithm; it never
changes which algorithm chooses a break. It calls textwrap's existing line-break engine directly.

The span cursor keeps a lower bound on its remaining width. It asks the existing width
helper for an exact suffix width only when that bound cannot answer whether a row fits.
After consuming text, it charges ordinary ASCII scalars one cell, other Unicode scalars
three cells, and tabs eight cells. The locked unicode-width 0.1 reverse fold adds at most
three cells per scalar; ASCII `#`, `*` and digits use the larger charge because they can
join emoji sequences. A remaining tab adds at most seven cells when the removed prefix
changes its tab stop. These are upper bounds on removed width, so the saved remaining
width stays a lower bound. The cursor never assumes that Unicode widths simply add.

The plain owner preserves the existing user-fence background, blank delimiter rows,
stripped fences for other roles and final empty logical line. It keeps fence state at
ordinary batch boundaries. Root can reuse `visit_ranges`, `SegmentCursor::position`
and `SourceRanges::text_with` for its existing retained Markdown spans without saving
another flattened Markdown string; this slice does not change Markdown parsing.
`PlainPages::from_offset` can skip a physical heading while retaining the full original.
It validates the UTF-8 boundary, starts fence state afresh at that byte and keeps the
same offset when text grows or width changes. A skipped-fence-header test checks this.

## Checks

The focused tests compare text, styled spans, cells, roles and soft-wrap flags with the
existing complete formatter at 8/12/28/80/120 columns. They cover ordinary and oversized
lines, append, resize, control-only style changes, text equal to SGR digits, OSC controls,
styles inside joined graphemes, tabs and trailing spaces. Plain tests also cover each
message role, fences across batches, CR and final empty lines.

The cursor tests compare the borrowed consumer with the old owned-suffix consumer.
They include Arabic, Hebrew, Tifinagh, Buginese and Lisu ligatures, regional indicators,
keycap emoji, variation selectors and deterministic mixed cases with changing row widths.
The new late-Unicode/tab test bounds whole-suffix width scans to less than three times
the source bytes. Replacing the tightened charge with the old eight-cells-per-scalar
charge makes that test fail; restoring the final helper makes it pass. Raw logs are
`/tmp/carlos-ep019b/long-line-cursor-negative.log` and `long-line-cursor-restored.log`.

The isolated checkout is `/tmp/carlos-ep019b/long-line-checkout`, from
`f093f15dc3e0f3eff07e343dd1e6adcc194e735e` plus this slice's helper/test files.
Its Cargo target is `/tmp/carlos-ep019b/long-line-target`; it never shares the primary
target. The final source manifest lists every changed/new file and executable hash.
Root's later task-panel, source-cache and Markdown edits are not part of this isolated
formatter executable. Its archived capture hydrator still validates every recorded
chunk through `Bodies` and then appends that chunk's text. It does not use root's later
accepted-piece caller or live owner. Full terminal checks belong to root's combined source.

## Discarded measurements and repairs

The first long-line executable, SHA256
`320c0036ac8be486249f24e7dad12a7da774b5c80612c2f21454b1f17baca0c9`,
drew saved 50-row windows in roughly 0.005–0.03 ms across 1–64 MiB fixtures.
It was rejected because the tab map kept two 24-byte runs per tab, vectors retained unused
slots, ordinary checkpoints carried the complete long-index headers, and ordinary batches
allocated unnecessary original-range metadata. Its ordinary captured-source append p95 rose
from 0.531 to 1.421 ms at 28 columns in that diagnostic pair. Its Unicode/tab fixture kept
486540000 bytes of index capacity for a 64 MiB source. Do not cite that executable as accepted.
Its files remain under `/tmp/carlos-ep019b/long-line-measurements/` and its runner log is
`/tmp/carlos-ep019b/long-line-measurements.log`.

The next diagnostic executable, SHA256
`a73a0288ea39d1781328c8c7ae41efe53dcb29bd0b5e5aa778cdfa64bb1d931f`,
used the compact tab map and boxed long indexes. Its component test found another avoidable
cost after appending Unicode and a tab to a 64 MiB ASCII line: the old conservative charge
made the cursor scan 515089362 suffix bytes and spend 8556 ms consuming styles. The parser
took 93 ms, the map 865 ms, and textwrap plus row-cell collection 1833 ms. Before that append,
the ASCII cursor took 61 ms and scanned zero suffix bytes. These are direct component timers,
not times subtracted from another run. The component logs and exact source hashes are under
`/tmp/carlos-ep019b/long-line-components-before/`. The final charge fixes this measured cause.

The pre-fragment candidate, SHA256
`6110939d85da68aaacb6a59747479e46da6ce1fab53e2d73317d77790109784e`,
kept the old public `wrap()` allocation path for all lines. It reduced retained metadata
and the late-Unicode cursor cost, but the 64 MiB Unicode/tab case still reached
2062700 KiB high water after text grew or width changed. Root kept general-long-line
production promotion gated and authorized the compact-fragment helper above.

The 24-byte diagnostic helper, SHA256
`c6d663530ffd3fbbbec0f135170736ad5a5dba8c160724e49a112a08f55a53bb`,
kept exact breaks but still left a worthwhile eight bytes per ordinary word unused.
Its final-source replacement uses the checked 16-byte representation and wide fallback.
Its diagnostic records remain under `/tmp/carlos-ep019b/long-line-optimal-components/`.

## Final measurements

The final release test executable is `/tmp/carlos-ep019b/long-line-packed-test`, SHA256
`dea828b09f3320923c0620a13d0d7eb281f89ae1fbdd8c543a926a3863b3ad03`.
It was built with `cargo test --release --no-default-features`. Its 20 source files
match the primary checkout byte-for-byte. The manifest, fixture hashes, sample counts
and complete per-case outputs are under `/tmp/carlos-ep019b/long-line-packed-measurements/`.
The probe checks every synthetic original with BLAKE3 before construction and after
text grows and width changes. The two real-capture original-getter checks also pass;
`source-getters.json` records the wire SHA256 and both original-string fingerprints.
These checks exercise the borrowed getter, not the terminal's Ctrl+Y route.

Each synthetic case runs in a separate process on CPU 6. The four general ANSI fixtures
use 1/4/16/64 MiB lines; plain fixtures use 16 MiB and the literal uses 64 MiB.
The probe indexes at 28 columns, reads 101 viewports across the whole row range, appends the
same Unicode/tab suffix and then changes width to 80 columns. A saved literal word instead
appends alphanumeric bytes. The viewport p95 is the 95th-percentile elapsed time from
those 101 samples. Long-line append/resize figures below are single diagnostic samples.
The probe records resident process memory (RSS) and Linux's largest recorded resident size
(VmHWM) in KiB; index capacity counts exclude the original string and painted rows.

- The 64 MiB punctuation/words line indexes in 1.771 s, appends in 3.105 s and resizes
  in 3.241 s. Its 50-row window p95 is 0.00571 ms. It keeps 114228328 index bytes at
  28 columns; RSS after indexing is 185364 KiB and final high water is 684000 KiB.
- The 64 MiB Unicode/tab line indexes/appends/resizes in 6.068/6.093/5.946 s. Its window
  p95 is 0.02254 ms. It keeps 167336848 index bytes, versus the rejected 486540000;
  RSS after indexing is 237684 KiB and final high water is 1335352 KiB.
- The 64 MiB mixed SGR/Unicode/tab line takes 3.740/3.676/3.927 s, with window p95
  0.05825 ms. It keeps 248794280 index bytes, versus the rejected 452985408;
  RSS after indexing is 419432 KiB and final high water is 1233204 KiB.
- The 64 MiB ligature/emoji line takes 2.084/4.349/4.434 s, with window p95 0.01292 ms.
  It keeps 31396448 index bytes; RSS after indexing is 137352 KiB and final high water
  is 427000 KiB. The data include Arabic/Hebrew/Tifinagh/Buginese/Lisu combinations.
- The 16 MiB plain words case takes 406/425/518 ms, keeps 17134440 index bytes and
  draws 50 rows at p95 0.00439 ms. Plain Unicode/tabs take 991/972/974 ms, keep
  34861984 index bytes and draw at p95 0.01133 ms.

The complete 1/4/16/64 MiB records show roughly linear indexing/rebuild work and stable
viewport work. The helper saves rows rather than walking a preceding source prefix for
each viewport. It still does complete-line work when textwrap can change earlier breaks.
The small differential tests check complete text/styles/flags against the old renderer;
the large synthetic diagnostics compare row/word counts with the old public wrapper,
without running its quadratic owned style consumer on a pathological large line.

### Where the temporary peak comes from

`/tmp/carlos-ep019b/long-line-packed-components/` holds per-phase RSS/high-water records
for the final executable. Reference mode selects the prior public `wrap()` path; both
modes use the new source map and style cursor. These diagnostics isolate word layout,
not the old complete formatter. They deliberately keep their flat input and row-cell
vector for the separate style-cursor timer, so their peaks differ from the complete-model
probe above. All phase timers exclude their `/proc/self/status` reads.

After the suffix append, the 64 MiB Unicode case has 18302419 upstream words and exactly
the same rows in both modes. RSS before wrapping is 378524/378800 KiB for reference/compact.
High water after wrapping is 2094968/1379684 KiB; wrap time is 3261/3331 ms. The 64 MiB
mixed case drops 1467684 to 1082312 KiB, with 9820810 words and the same rows. The word
representation improves the peak; it does not promise faster optimal-fit calculations.

The locked textwrap 0.16.4 code keeps full arrays for width sums (`f64`), line numbers
(`RefCell<Vec<usize>>`) and best-break/cost results (`(usize, f64)`). Its SMAWK routine,
which finds the cheapest breaks, also allocates row scratch for 3N indices, column scratch
for 2N indices and another N indices for minima. Some zeroed scratch pages stay unresident.
The helper reuses those routines unchanged, so these arrays still grow with the number
of word breaks. The final code improves the old path; it does not claim constant-memory
optimal wrapping. Root must assess the remaining 1.3 GiB complete-process peak before
promoting the general-long-line path.

### Preserve the accepted ordinary and literal paths

The accepted comparison executable remains SHA256
`731c23831eee366b7afcfb0703225636110bcc89a1fdd80920e32a985c50c739`.
Both executables replay the same earlier capture hydrator and private clipboard guard.
The ordinary capture has SHA256
`1845226d523271fc15e979d320742aa0f2d29d558c6be8eb350bce39bcbbdf9d`;
the literal capture has SHA256
`cbb76345e6d96d7dc3b7d4168f51c034cb1cface04b2feccca072b3598dcc09f`.
All current copies use fresh run-specific paths, and the runner verifies each complete
source digest. These remain current bodies in unfinished native turns, not completed
64 MiB native command items.

Five alternating process pairs on CPU 6 preserve all captured row counts at 28/120 columns.
Each process takes 16 append/resize samples, so its p95 is its largest sample. The following
values are medians of those five per-process p95 values; append/resize include one process
status read, while the initial index and viewport timers exclude that read.

- Ordinary append p95 is 0.516/0.363 ms versus accepted 0.510/0.358 ms; viewport medians
  are 0.182/0.021 ms versus 0.180/0.023 ms. Resize medians are 101.16/99.21 ms versus
  99.24/99.39 ms. Initial indexing is 118.10/119.95 ms versus 112.51/112.07 ms, an added
  5.6/7.9 ms in this diagnostic. The index keeps 589824 bytes versus 524288.
- Literal append p95 is 0.328/0.316 ms versus accepted 0.316/0.330 ms; viewport medians
  stay about 0.002 ms. Resize p95 stays about 0.016–0.018 ms. First indexing improves
  to 147.60/148.02 ms versus 279.62/278.86 ms. The index keeps 288 bytes versus 256.

The raw pairs and summaries are `/tmp/carlos-ep019b/long-line-packed-pairs/`.
These diagnostic pairs support the existing sub-millisecond hot paths; they are not
matched end-to-end socket latency confidence intervals or a claim of identical initial cost.

The isolated workspace passes 805 main, 75 client and 347 core tests plus protocol,
daemon and architecture targets. Strict workspace Clippy passes. All 20 files stay below
400 lines and every function below 60 lines. The exact measured release also runs the
focused formatter tests; root owns the combined default-feature and terminal checks.

## Remaining work

The formatter must still revisit a complete changed logical line when textwrap can choose
different optimal breaks after append or resize. Root must run that work away from the input
thread and fence stale results against the source revision and display choices. The tests
here do not prove socket input latency, terminal copy/selection, reconnect behavior or the
production above-cache source path. They do not justify a file mapping or another parser.

## Engineering review

The bounded `codex exec --ephemeral` review returned PASS without findings. The reviewer
independently checked all source/executable hashes and reran all 17 focused tests from
the exact measured release. Raw files are `/tmp/carlos-ep019b/long-line-engineering-review.log`
and `long-line-engineering-review.txt`; the prompt and concrete subject are saved beside them.
Its complete verdict follows.

Reviewed the 20 scoped source/test files against base `f093f15`, the report, saved measurements, and locked library code.

- **Findings:** No BLOCKER, MAJOR, or MINOR defects found. Saved rows avoid preceding-row walks; compact records preserve textwrap’s break inputs; source getters and plain suffix offsets meet the stated requirements.
- **Checks:** All source hashes and the executable hash match the manifest. I reran the measured release executable: 17 focused tests passed; two diagnostics remained ignored.
- **Report:** The saved evidence supports its reported figures and limits, including the roughly 1.3 GiB peak and multi-second complete-line rebuilds.
- **Corrective guidance:** No changes required within this review’s scope. The report correctly leaves production use gated on those remaining costs and root’s integration checks.

**Verdict: PASS**
