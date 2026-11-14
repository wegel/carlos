# Whole Markdown documents and accepted original bytes

EP019b remains incomplete. This checkpoint measures one parsed Markdown document,
keeps only requested painted rows, and copies bounded original bytes from the
shared client's accepted cache. The live terminal does not use this large-body
path yet. The installed executable and main branch stay unchanged.

## Code and limits

`transcript_markdown_document.rs` calls the existing whole-document parser, saves
styled logical lines, and counts wrapped rows in a prefix index. Its window
method paints touched logical lines with the existing painter. A single long
logical line can still paint many rows; it needs a saved row index before this
prototype can be promoted. The normal test matches old text, styles and row
flags at8/12/28/80/120columns, including late references, tables, nested lists,
quotes, fences and Unicode. No streamed parser or changed wrap algorithm was added.

The only production change reserves the exact number of logical lines and
nonempty spans in `transcript_styles.rs`. Blank lines reserve no span slots.
The parser, text, styles and painter stay the same. Full rich-document raster
digests match the old renderer at28/120columns.

`accepted_source.rs` copies at most64KiB after the cache has checked the read
token, revision, cursor and offset. It rejects retired/replaced windows, missing
prefixes and starts inside UTF-8 characters. Each piece ends at a character
boundary. A missing prefix requires an explicit seek. Three normal tests cover
reconstruction after256KiB cache eviction, stale reads and Unicode boundaries.
The probe moves raw chunks into the shared cache and copies only accepted
pieces. No extra pure-client API or larger cache was needed.

## Captures and memory

The earlier actual-daemon16MiB Markdown capture is mostly a code fence. The new
actual-daemon4MiB rich capture has headings, paragraphs, inline styles/code/links,
nested lists, quotes and tables, with its reference definition at the end. It
arrived in65 UTF-8-safe reads, SHA256
`06af865e844ab8ac58b621d13f6fc2db24e95d73601f9fa366fd4063e1c04872`.
The manifest and chunks are in `target/large-source-rich`.

Before exact reservations, the rich parsed document retains103,219,475bytes of
Vec/String capacity; afterward61,138,795bytes. Its row index retains2,765,432
bytes at both widths; the final50 painted rows retain15,570/18,114bytes at28/120
columns. RSS after indexing falls from147280/145312KiB to130496/130112KiB.
Parser peak memory remains about200MiB; this does not solve every parser cost.

The fenced16MiB document's retained line capacity falls from56,413,650 to
29,989,482bytes. Its index retains1,677,744bytes. RSS after indexing falls from
98868/98112KiB to61476/61464KiB. This corpus differs from rich Markdown.

The isolated16-sample after-reservation rich measurements show parse270.40/
266.89ms, index124.36/53.16ms, and final50-row paint.0470/.0648ms at28/120columns.
Whole rebuild append p95 is386.40/306.20ms and resize p95 is372.43/372.12ms.
Fenced append p95 is31.41/31.22ms and resize21.84/22.26ms. These are offline
formatter costs, not socket/input latency. Whole parses must stay outside the
input loop. An earlier run overlapped a release compile is excluded from latency
comparisons; its log remains `markdown-rich-reserved-document.log` under
`/tmp/carlos-ep019b`.

Before files are `preallocation-document-{28,120}.json` in each capture
directory; afterward they are `document-{28,120}.json`. Final log:
`/tmp/carlos-ep019b/markdown-reserved-final-measurements.log`. The full rich
raster after-check used one sample only for row digests, not latency comparison.

## Matched runtime performance harness

Both frozen runtime builds use `--release --no-default-features`, aad061e source,
and differ only in the eight added/two removed reservation lines. Before:
`/tmp/carlos-ep019b/markdown-before-carlos`, SHA256
`8947b074c07a5fe0acc52185104c930bdc05c17a7f3972c5f2dc4fc5f2728f7f`.
After: `/tmp/carlos-ep019b/markdown-after-carlos`, SHA256
`d49d4775e03e3dc000046278e5682a9894fcacd359498f1592894f428f62b7fd`.
Neither was installed. The script runs15 rotated pairs on CPU6,160x48 terminals,
private homes and each executed process's own high water mark. It calls
`carlos perf-session` on a synthetic1000-turn/24-tool-line session and a fixed file.

Synthetic counts stay6001messages/39347lines/6000events. Median layout changes
14.76to14.81ms; scroll p95 stays.82ms, typing.61to.60ms, append stays.67ms.
Paired append changes+.01ms, interval[0,.02]. Fixed counts stay5/9/4; scroll
stays.44ms, typing.39ms, append.48to.47ms. Small-case peak RSS increases:
21704to22612KiB synthetic and9120to10108KiB fixed. Paired increases are1020KiB
[772,1144] and952KiB[800,1120]. Accept the roughly1MiB small-case peak increase
for42MB less rich line storage; do not claim universal memory improvement.
Samples, identities and intervals: `/tmp/carlos-ep019b/markdown-runtime-latency`.

## Accepted-source replay and checks

The final producer test uses `--release --no-default-features`, SHA256
`4a08a3c963e61f43f55b6b750ea3550cb58acc5733fd327b032dc4a840945cf5`, frozen at
`/tmp/carlos-ep019b/accepted-source-producer-test`. Source hashes and profile:
`target/accepted-source-proof/producer-identity.json`. All eight records pass:
64MiB ordinary ANSI,64MiB single-word ANSI,16MiB fenced Markdown,4MiB rich
Markdown, each at28/120columns. Every run requires a fresh metrics UUID and
successful private clipboard program with the exact source digest. The one-
sample replay proves reconstruction/copy, not comparative speed. Inputs point
to preserved captures; new outputs live under
`target/accepted-source-proof/{ordinary,word,rich}`. Log:
`/tmp/carlos-ep019b/accepted-source-capture-replay.log`.

The default-feature release gate uses an isolated accepted-code checkout with
its own target, the accepted task-client/range fixes and root's source-seek fix,
excluding the task worker's partial code. `bash scripts/test.sh --release`
passes1292Rust tests/7ignored,53 daemon workflows,219 terminal workflows,
67 helper checks,15 architecture checks, package smoke, strict Clippy and fmt.
Log: `/tmp/carlos-ep019b/markdown-accepted-source-release-checks.log`.
Artifacts: `/tmp/carlos-ep019b/markdown-runtime-target/ui-artifacts/run-BAVXjZ`.
No native Codex protocol changed in this checkpoint.

## Engineering review

The separate bounded read-only reviewer returned PASS without findings. It reran
all four focused tests and independently checked source/frozen binary hashes,
eight fresh copy digests, raster digests and reported memory figures. This cannot
approve EP019b. Next connect
one platform-owned original and formatter to actual socket reads, preserve
exact copy/scroll positions, and test cache pressure, growth, resize and reconnect.


Raw reviewer output (`/tmp/carlos-ep019b/markdown-source-review.txt`):

Reviewed only the listed Markdown/source-copy files and the proof report.

- **Findings:** No BLOCKER, MAJOR, or MINOR defects found.
- [transcript_styles.rs](/var/home/wegel/work/perso/carlos-daemon/src/app/transcript_styles.rs:51) reserves vector space without changing parser output.
- [accepted_source.rs](/var/home/wegel/work/perso/carlos-daemon/src/app/accepted_source.rs:35) checks stale tokens, revisions, missing prefixes, byte offsets, and UTF-8 boundaries.
- [transcript_markdown_document.rs](/var/home/wegel/work/perso/carlos-daemon/src/app/transcript_markdown_document.rs:85) selects viewport rows consistently with the existing painter. The report explicitly records the long-line cost and test-only scope.
- Re-ran all three accepted-source tests and the Markdown differential test successfully. Verified current source hashes, frozen binary hashes, eight distinct copy records and clipboard digests, matching rich-document render digests, and reported memory figures.

**Corrective guidance:** None required for this checkpoint. The evidence supports offline costs and exact copying; it does not establish live large-body support.

**Verdict: PASS**