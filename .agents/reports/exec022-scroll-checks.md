# EP022 phone scrolling checks

This checkpoint records the owner's report that scrolling both stutters and jumps.
EP022 remains open. Root made the changes directly; no intermediate reviewer ran.

## What changed

The transcript now uses UIKit's reusable collection cells to host the existing
SwiftUI message views. Each cell keeps the original message or document-block ID.
The app no longer runs a competing SwiftUI LazyVStack/ScrollPosition controller or
keeps weak native marker views. The shared Rust client, source readers and native
backend state remain the owners of conversation facts.

The collection controller holds structural changes through dragging and deceleration,
then applies only the newest pending transcript. Native layout accounts for cell
height changes above the reader. An explicit history/tool anchor includes the
message's top gap, which can change when older messages arrive.

A host-local NSCache retains parsed inline documents across cell reuse. Its count
limit is64 and its total cost hint is2MiB; these are eviction hints, not a measured
allocator ceiling. The app prepares up to16 upcoming entries. Parsed block facts
stay with their document, so reused blocks no longer start as22pt placeholders.
Code highlighting and document parsing still run off the UI thread.

Complete source answers already become separate collection rows. A transitional
message cell now retains only its preview until the parent installs those rows.
It must not first lay out the entire source in one cell. Full paragraph, heading
and table text can grow vertically instead of being truncated by the host cell.

## Retained failures

Native sources4a9419d2471c4f42f9e6 and02450b8f5ff3e9cf4163 shifted the original
message36pt after a failed history read. Sourcea8f5fe32d7ade0a8d7b4 fixed that check
but shifted the message12pt after older rows arrived. The message's changing top
gap caused the latter error. All original2pt coordinate limits remain in force.

Sourcec4aa20ff006b6a727aac passed history, tool/live-update anchors and the short
private reference checks. Its signed app was installed on the owner's iPhone18 Pro.
The owner still reported stutters and jumps while moving, so those settled-position
passes do not establish smooth motion.

Source982f15b87f37886beb7f reduced measured scroll CPU and passed history, tool/live
updates and10000-line code. Its10MB source check failed because XCTest could not
obtain a screen: the app's main run loop stayed busy for30s. A retained3s sample
shows SwiftUI text layout, and the screen remained on Loading original text.
The transitional single-cell source renderer caused the stall. The corrected
source9a45429810146c725110 passes the same10MB/final-text/exact-clipboard workflow
in48.0s, native fixturephone-codex-9eeafe3e2ae3, and10000-line code in36.5s,
fixturephone-codex-cb9fab330aa4. No deadline, copied bytes or assertions were weakened.

## Measured scroll work

The actual app replayed the owner's privately captured conversation over a read-only
fixture connection. It never contacted the owner's normal-shell daemon or submitted
a native prompt. Both builds used the same captured rows, Auto tools, and three
slow-down/fast-up gesture pairs on the owned iPhone17 Pro simulator.

Baseline44bfcd6e902bd588621d is the prior native scroller plus the measurement test.
Candidate982f15b87f37886beb7f adds the parsed-row cache and gesture guard.

- Mean CPU time:1.0336s before,0.8018s after,22.4% less.
- Mean physical peak memory:69040.8kB before,68494.7kB after.
- Mean scroll/deceleration duration:2.3544s before,2.3541s after.

These XCTest results contain no frame-rate or hitch metric. Gesture duration is not a
frame-rate measurement, and simulator CPU savings do not prove physical-phone motion.
The later source-row fix still needs its own final performance run.

Raw metrics and logs remain under /tmp/carlos-ep022/native-scroll-baseline-* and
native-prepared-*. The captured message bodies and screenshots stay in ignored,
private artifacts; they are not included in this report or reviewer context.

## Remaining checks at this checkpoint

Source9a45429810146c725110 also passes syntax/exact-copy in47.9s, history
failure/retry/prepend in32.6s, and tool expansion/live-update position in91.2s.
The original2pt assertions and native traffic checks remain in force. The full
private reference passes in56.8s in both layouts, without a mutation or rejected
read. All19 native units pass with0 failures/skips. The checked Release app was
strictly signed, installed and launched on iPhone18 Pro at16:07:51/54Z. The
immutable preview is previews/9a45429810146c725110-20261117T160640Z/Carlos.app on
the owned Mac; executable SHA256 is
0fb2975939ef081c3e37bc060290b59d13e59b810e146a58bdfc977fc4a29d6a.
The owner has been told it is ready to test. Direct physical motion feedback
now confirms this installed build still jerks. Root has paused the broad sweep
to capture moving frames before another fix; no smooth-motion claim follows
from the simulator checks. The complete functional sweep, final measurements and end-only
reviews remain required before EP022 can close. The existing full Linux release,
terminal and native-backend gate passes on runtime source92d3a53538102e52153236eb9c0b4ba0;
these scrolling changes are Swift-only, and a subsequent workspace cargo test passes.

The owner's normal-shell daemon and frozen engine binaries remain unchanged.

## Current source and package size

The authored phone Rust bridge contains2051 nonblank lines in19 files; the Swift
app contains3980 nonblank lines in50 files. These counts include comments and
exclude generated bindings and tests. Against accepted EP021b treefb5ed10, all
production groups together add6673 nonblank lines and remove141. The counter
walks reachable production Rust modules and excludes test/test-support code.
Raw counts and per-file changes are /tmp/carlos-ep022/production-count.json.

The signed9a45 executable is9085840bytes; the entire app bundle is11403998bytes
in17 files. The bundle includes the real fonts and syntax grammars. Its source
and executable hash are checked before installation.

The actual generated Swift-to-Rust calls also pass their five-sample unit
measurements:1000 checked Markdown block reads average30.74ms; formatting500
lines with the already-loaded Rust grammar averages82.88ms. These are simulator
unit measurements, not physical-phone frame timings. The grammar runs off the
UI thread in the app. Raw metrics: final-native-bridge-metrics-01.json.

## Moving-frame reproduction and correction

The owner confirmed9a45 still jerks and required Root to observe it before another
fix. The new read-only deep-history workflow scrolls six older screens and four
fast returns in both Flow and Gutter. Its6ffa movie contains clear backward snaps.
Exact decoded frames4144/4145/4146 show the same transcript moving upward16pt
during a downward scroll; three independent text patches match at>0.999. Nearby
frames show15pt and13.8pt reversals too. Random frame seeking through OpenCV did
not return the requested frames in this variable-rate movie, so only exact
FFmpeg decoded indices support these visual comparisons. Raw phase candidates
locate frames to inspect; they do not establish a verdict alone.

Direct numeric traces show cached answers were parsed once, followed by ready
cache hits. The custom preferred-invalidation callback did not run in this case.
The measurements also change legitimately between Flow and Gutter. The fault
appears when the native collection replaces old or estimated cell geometry while
the reader moves. Root removed all temporary cache/layout logs before testing the
correction. The corrected native view snapshots existing visible source cells
after the pan moves the viewport, lets UIKit lay out new sizes, and cancels extra
movement of a surviving cell caused by changed geometry. It skips elastic
overscroll and guards against recursive corrections.

Sourcea665e3dbaa349408efd9 runs the same deep gestures in82.97s. Its recording
has no opposite-direction candidate during scrolling. The single flagged event
is the settings sheet beginning to open, verified in exact decoded frames831
through833. Screens also confirm that the panel traverses older messages in both
layouts. The native/Rust/history checks pass; long-message and tool checks remain
pending. No physical-phone smoothness claim follows from this simulator result.

The actual before movie, after movie, decoded strips and numeric data remain in
ignored private artifacts. The temporary opt-in phone recorder6ffa is installed
but has not recorded: the owner has not answered its optional timing question.
Camera and microphone stay disabled; iOS consent is required. Remove the recorder
and its app launch hook before EP022 closes.
