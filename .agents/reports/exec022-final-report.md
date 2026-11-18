# EP022 iPhone app acceptance evidence

The iPhone app uses the same Rust client as the terminal. The daemon owns
sessions and native backends. Swift owns the interface, WebSocket and Keychain,
and calls a UniFFI bridge for checked session facts and commands. The bridge
exports the calls these screens need. Terminal-only panels stay in Rust. A
future web client can reuse the shared rules through its own bindings.

The phone saves configured servers and reconnects with Keychain credentials.
Users can send, steer, queue, stop, answer approvals, rename supported sessions
and explicitly fork from an older prompt. Drafts stay on the device. The app
reads tool output in bounded pieces and can expand a running tool when it has
output. Flow and Gutter both use native list scrolling. Done and Mic are on
the left of the keyboard bar. Tapping the log also hides the keyboard.

## Checked source and release pair

Phone build aba92b522eddbb638cb3 includes the left-side controls from b9d42e81,
readable fork errors from f065b789 and the large-document prefetch change from
86c69af4. Commit 6e15e9d4 corrects an assertion in the Linux fixture and leaves
the phone source unchanged. The simulator and device builds use the owned
Intel Mac with macOS and Xcode 26.0.1, Rust 1.100.0 and pinned Apple targets.

The end review found that handled private failures could fill the bounded
operation store. The correction is d8c2e16f, with the final simulator fixture
from f168cd31 in phone bundle 35f2cd499401ff683df5. The new Swift bridge test
rejects 129 textless controls and then successfully admits another prompt.
The actual app restored an uncertain prompt without resending it. It also
discarded the retained text of a rejected fork without replacing the main
draft or sending any native prompt. The Linux package source is unchanged.

The live daemon runs e3fde911dc437d90e38e611d97ec5812. The release package
under target/ui-artifacts/run-oen0FO/package passed 226 daemon and 336 terminal
cases, the source, selection, ANSI and dictation suites, 23 native Codex and
10 packaged Claude cases, and the installed Codex contract. The last Swift
changes did not alter those bytes. Workspace Rust tests and the full debug
terminal suite passed after the phone changes. The debug gate keeps its 226
daemon and 336 terminal results under target/ui-artifacts/run-91qvAq.

The final Linux build, a50bd0d31e4fe0da61a4be0cb3d22fdc, passed that full
release gate again under target/ui-artifacts/run-YWgEbR. The command was
`bash scripts/test.sh --release --codex-contract --claude-contract` with the
pinned Bun directory on PATH and BUN_BINARY pointing to its executable.
The terminal binary matches the measured bytes and all 515 package hashes
verify. The final socket timing uses this package's actual daemon binary.

The owner-local install updated `~/.local/bin/carlos`, `carlosd` and the Claude
controller together from this package. The rollback receipt lives under
`~/.local/lib/carlos/backups/20261118T031016Z/install.json`. The running daemon
still uses e3fde911 and serves the active connection. The final Linux changes
affect the terminal's wrap count and the test harness. Both daemon builds use
the same protocol. The compatible package can start on the next normal daemon
launch without interrupting this turn.

The idle-upgrade receipt proves the saved phone token authenticates, the
working session is still listed and the Mac reaches the LAN listener.

The performance baseline is the original EP021b release, source
67a1a38a348a6343400594a6d06dbe4a. The preserved target/phone-preview-b7744b8
executables match all three hashes in exec021b-performance-reviewed.json.
The paired benchmark uses verified copies in
target/ios-artifacts/ep022-final/ep021b-pair.

## Phone evidence

The exact source passed 32 functional simulator workflows, four measuring
workflows and 26 unit tests. The keyboard check covers Steer, Queue, Send,
Stop, Done and tapping the log. The signed release installed on the iPhone
18 Pro. The phone was locked, so iOS denied the automatic launch. The signed
app and its verification receipt are preserved on the build Mac.

On the simulator, the median first responsive frame took 2.138 s. The typing
test took 1.448 s including XCTest input and assertions, used 1.057 s of CPU
and reached 66,974 kB of peak physical memory. The scroll interval took 2.350 s
including gestures and coasting, used 0.693 s of CPU and reached 68,346 kB of
peak physical memory. A separate twelve-second idle sample showed 0.083% CPU
and 259,864 KiB RSS. The resident set and XCTest physical memory counters
measure different aspects of the process. The bridge took a median 82.1 ms
for a 500-line Rust syntax example and 30.8 ms for 1,000 checked Markdown
reads. These are simulator measurements. The raw results are under
target/ios-artifacts/ep022-final.

On identical 10 MB input, both the previously installed build and the final
candidate copied every original byte. Sampled peak app RSS fell from 1,138,628
to 599,580 KiB. This is a simulator UI test and includes XCTest accessibility
and clipboard work. UIKit keeps regular prefetch for transcripts without a
rendered Markdown document of at least 1 MiB. The normal flick check passes
its existing 10% limit against the native scroll view at both speeds.
Deep scrolling, live tools, history retry and prepend, and the 10,000-line code
case pass on the same build.

The 76-row real Claude capture ran through the actual connection flow in 56.994
seconds. The private read-only replay served three admitted reads and received
no unexpected requests. Both layouts kept the fixed controls painted and older
text in place. The capture predates the typed link between tool calls and
results. The current native Claude fixture separately checks that relationship.
Private conversation data and screenshots stay outside Git.

The initial history run passed XCTest but its harness expected a native offset
of 100 turns. The fixture now requests one turn per call. The corrected harness
confirms that the failed read and retry use the same original one-turn cursor.
The passing rerun and original failure are in target/ios-artifacts/ep022-final
as history-recheck.log and history.log.

## Build costs and authored size

The unsigned device release app contains 11,959,811 bytes, including a
9,658,904-byte executable and 2,063,860 bytes of bundled fonts. The generated
XCFramework has 158,623,678 bytes across its device and dual-architecture
simulator static archives. Those archives are build inputs. The app includes
the linked device executable. Generated Swift is 205,503 bytes.

Compared with EP021b, the app adds 4,600 nonblank Swift lines and the Rust phone
bridge adds 2,240. Shared client code adds 523, Core adds 111, the protocol adds
103 and the daemon adds 81. The terminal removes 37 and the Claude controller
stays the same size. The net increase is 7,621 nonblank lines. These counts
exclude named test modules, test fixtures, generated bindings, resources,
build scripts and docs. Component totals are in
target/ios-artifacts/ep022-recovery-checked/source-count.json.

## Terminal and daemon measurements

The first paired replay found an extra 7.38 ms per cold load (60.88 ms versus
53.50 ms for EP021b). The terminal's wrap counter measured widths that textwrap
had already measured. It now uses those widths directly. For ASCII letter
runs with attached periods, commas and exclamation or question marks, it
builds the same word records in one pass, including widths and spaces. Both
paths use the same optimal-fit algorithm. Tests compare word records and
counted versus painted rows. Code, numbers, ANSI and Unicode keep the general
word separator.

Fifteen alternating runs on CPU6 compare the original EP021b pair with
source a50bd0d31e4fe0da61a4be0cb3d22fdc. Both replayed 6,000 relevant items,
kept 6,001 messages and rendered 39,347 rows. Median cold replay was
53.39 to 53.46 ms, inside the original EP016 54.20 ms limit. The median of
the runs' scroll p95 values was 0.70 to 0.69 ms, typing p95 was 0.49 ms on
both and append p95 was 0.56 to 0.55 ms. Separate peak RSS samples were
25,696 and 25,708 KiB. Five runs of the captured replay fixture kept the
same five messages, nine rendered rows and four relevant items, with
0.87 and 0.93 ms median cold replay. The measurements and hashes are in
target/ios-artifacts/ep022-final/linux-perf-review-pair.

The socket timing harness waits for a full newline-terminated address after
binding port zero. The first run read the log partway through that line and
tried to connect to a truncated hostname. The final run uses the same fixture
for both packages.

Three runs of 1,000 admissions per socket type gave median per-run p95 of
0.618 to 0.612 ms on Unix sockets and 0.793 to 0.799 ms on WebSockets.
Time to bind the fixture listener was 2.38 to 2.19 ms on Unix sockets and
2.35 to 2.39 ms on WebSockets. Every idle sample used zero process CPU ticks.
Three exact 10,092,544-byte source transfers per socket type verified the
original SHA-256. Median wall time was 115.83 to 116.93 ms over Unix sockets
and 129.66 to 128.11 ms over WebSockets. Raw distributions, daemon memory
samples and executable hashes are in linux-perf-review-pair/measurements.json
under the same artifact root.
