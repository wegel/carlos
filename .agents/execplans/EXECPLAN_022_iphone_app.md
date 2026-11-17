# EP022: build the actual iPhone client on the shared Rust client

This living ExecPlan follows `.agents/PLANS.md`. Keep Progress, Surprises & Discoveries,
Decision Log and Outcomes & Retrospective current. Root works directly on `iphone-spike`
in `/var/home/wegel/work/perso/carlos-daemon`, without developer subagents. The owner
delegated approval after complete checks, Root's engineering review and one end-only
Fable5.2 medium whole-roadmap review. Do not request intermediate reviewers or routine
approval. The owner's later normal-install request authorizes installing a fully checked,
matching CLI/daemon/helper package. Do not merge into main, alter an archived plan or replace
any frozen owner preview. A normal upstream push was explicitly requested separately;
that request does not authorize merging, force pushing or publishing an app.

## Purpose / Big Picture

A person should open Carlos on an iPhone, connect to their daemon, choose or create a
session, read its complete conversation, send a prompt, answer its actual approval cards
and interrupt work. They should see new text without losing their place or private draft.
The phone must use the same checked Rust client as the terminal, so Swift does not grow
another JSON-RPC controller, history merger or native-backend parser.

Build the real SwiftUI app from `tmp/main-screen.md`, using
`tmp/main-screen.prototype.dc.html` as a visual/behavior reference. Do not port the
prototype's HTML or scripted replies. EP018a supersedes its ban on expanding running
tools. Actual native choices supersede its assumption that every backend offers
session-wide Allow. An interrupt must display actual acknowledgement/facts, not fabricate
a successful stop or denial. Keep complete supported source, honest unknown outcomes,
private device text and original native guards.

The deliverable includes a narrow UniFFI bridge, an XCFramework (a bundle containing Rust
libraries for Apple targets), a buildable native app, deterministic simulator checks and
measured evidence. UniFFI generates Swift calls for Rust objects and records. Swift owns
the WebSocket, layout, scrolling, microphone, clipboard and device preferences. Rust owns
protocol checks, reconnect/reply rights, session facts, body/history reads and Markdown
structure. The phone must not link `carlos-core` or ship a backend/helper process.

## Progress

- [ ] Fix the owner's Codex resume failure before the remaining phone setup. After
  the sandbox restart, installed Carlos closes Codex on a stdout frame above64MiB;
  native Broken pipe is a consequence of that close. The current native session
  log is431MB with25 task-started turns, including large long-running turns.
  The initial native history request asks for100 full turns. Native full replies total
  83618724bytes; the largest individual turn is47477424bytes. The resumed thread
  reports paginated history and its latest turn reply is243930bytes. Carlos now
  requests one complete turn per reply without changing either frame cap or the
  original-request64MiB budget. Cached partial prefixes satisfy other readers,
  rather than fetching another turn to fill a maximum row count. Rust tests pass;
  real terminal/native checks, release gate and installed-pair update remain.
  Isolated copies of the owner's rollout and size-only probes are in ignored
  tmp/codex-large-resume; no prompt is sent and the original session is untouched.
  Codex0.169.0 schemas are generated there. Metadata-only read does not hydrate
  native item pages; the isolated resume probe now seeds them first.

- [ ] Resume after the owner's requested sandbox restart. The owner offers writable
  access to the real Carlos binary/config/state/runtime directories and asks Root
  to stop at a convenient checkpoint. No tests/builds remain running. Current host
  management source23952a4b8d31245275a0 is saved but not deployed. Source713e524dd1050306d293
  compiles and passes24 native units and1795 Rust tests (12 ignored). Both new UI
  runs reach remove confirmation, then fail on an ambiguous XCTest selector for
  nested Remove buttons. The selector now uses host.confirmRemove.firstMatch;
  that correction is staged in23952 and has not been rerun. Preserve both failed
  result bundles. Read .agents/reports/exec022-restart-handoff.md before resuming.
- [ ] Finish saved-server management. Server headers now show name and address
  with Manage/Edit/Remove, and Add server is visible in the Sessions toolbar.
  Address edits retain their saved ID and authenticate on a separate model/client
  before replacing a working connection; a changed address requires its own token.
  Equivalent addresses cannot add another entry, and a removed editor cannot save
  late. Removal clears only that entry's token/session preference and disconnects
  its local client. Other hosts/native sessions remain. Native tests cover these
  boundaries. Actual hosts/host-duplicates UI checks and credential regressions
  must pass before a new signed phone install. Keep accepted native scroll physics.
- [ ] Enable phone access to the owner's new NORMAL daemon after restart. The
  owner says it started automatically from Carlos and is idle. That default has
  only a Unix socket; it needs an explicit --listen IP:PORT and a device token.
  Keep the preview daemon on33731 untouched. Use a separate port such as33732
  for normal Carlos, the same normal socket/catalogue, and the real normal-shell
  daemon. No restart, token grant or setup script has been performed/prepared yet.
  Do not start the user's normal daemon from the tool container. After mounts are
  corrected, inspect actual paths and prepare a concrete normal-shell handover.

- [x] Verify the normal local installation is now visible. Installed~/.local/bin
  carlos/carlosd/helper hashes match the checked source92d3a535 package exactly:
  7167fb9c... / fefa8834... / 9b201307... . The owner is running its automatically
  started daemon. The tool's bin mount remains read-only until the requested
  sandbox restart. Earlier failed copy attempts changed no installed files; the
  shared tmp/install-carlos-92d3a535 installer/package/backups remain preserved.
  The installer saves its normal-shell receipt/backups under~/.local/lib/carlos;
  inspect that real directory after it is mounted, not the private overlay home.
- [x] Measure the prefetch-only large-source probe39c0e5c6fadc2526a385. It passes
  the identical10MB/exact-copy test47.51s and samples447560KiB peak RSS versus
  de74's1126612KiB, about60% lower. Keep no-prefetch-source-01, memory-01 and
  source-memory-39c0...json on the Mac. The working tree and installed phone still
  keep the owner-accepted normal prefetch setting. This implicates native cell
  prefetch in the stress-case cost; do not add an unrelated document cache.
  After the normal-install handover, consider disabling native cell prefetch only
  for oversized source documents, keeping ordinary history unchanged, and verify
  the actual source/scroll checks before adopting it. Final sweep/reviews remain.

- [x] Owner validates the installed native-list app on the actual iPhone18 Pro:
  "ah yes it's awesome now, thanks". The reported jerk and slow-fling problem is
  accepted on the physical device as well as covered by the recorded/native checks.
  Keep this normal-session behavior while finishing the remaining EP cost/review work.

- [x] Install the checked native-list preview de74 on iPhone18 Pro at19:03:50Z.
  Signing/strict verification succeeds; executable SHA256 is
  ec790a19d6fb7ffe25cabb2754aa19377fcaaf3da11a0402e0d3dca35fb241ca.
  Immutable Mac bundle:previews/de74af3e4ef7529f93d6-20261117T190305Z/Carlos.app.
  Install receipt reports success; auto-launch is denied because the phone is
  locked. The owner can open it normally. No phone recording was started.
- [ ] Check one bounded large-source memory hypothesis before the final sweep.
  Separate uncommitted snapshot39c0e5c6fadc2526a385 disables only UIKit prefetch;
  the working tree and installed de74 keep the proven normal setting. Run the
  identical source/exact-copy case and sampled RSS observer in no-prefetch-*-01.
  Use the result to distinguish speculative cache changes from a measured cause.

- [ ] Explain the measured large-source memory cost before final EP approval.
  The clean de74 10MB original-source/exact-copy workflow passes47.60s, fixture
  phone-codex-0abdb961c675, but its sampled app RSS peaks1126612KiB and remains
  1120856KiB just before termination. Keep source-memory-de74...json (176 samples).
  This is a stress-case app-process RSS sample during actual XCTest/clipboard/
  accessibility reads, not a normal-session footprint or physical-phone measurement.
  Candidate799 sampled926036KiB. During the final cost pass, distinguish large
  text layout/retention from XCTest accessibility work before changing caches.
  Do not hide this cost or add speculative caching without evidence.

- [x] Check recorder-free de74 native units19/19 and actual history failure/retry/
  prepend. Live-tool scrolling/expansion also passes95.01s, retaining all2pt limits.
  The native-list-final-gates runner continues with exact10MB source/copy and
  10000-line code. The passing flick candidatea1 differs only by the removed
  inactive recorder/task; all scrolling and fixture/test files match byte for byte.
- [x] Check the public generated-history native-flick fixture. Clean de74 passes
  97.34s: normal608pt versus612pt native; strong2478pt versus2467pt. The identical
  slow-setter baselineb09 fails at0.28798x native travel. This repeats the real
  regression without a private capture and checks one resumed owner/zero prompts.
  Native-motion uses the same generated history and is still pending its first
  public run. Keep native-list-synthetic-flick-01 and native-setter-synthetic-baseline-01.

- [ ] Finish the native list checks before another phone install. Source01dd58ff5c166397ace0
  replaces flow layout and removes manual gesture-offset corrections and their
  unused owner callbacks. Keep stable IDs, parsed documents, diffable updates and
  explicit idle history/tool anchors. Its102.04s deep replay passes. The exact
  sequential6125-frame movie reference-native-list-motion-01dd-01.mp4 has no
  opposite-direction candidate with the same confidence/movement rule that found
  the earlier failures. The overview shows genuine travel through older messages
  in both layouts, not a frozen viewport. The first raw-offset flick comparison
  fails at1.151x native travel; raw offsets also include revised row estimates.
  Sourcea1cd46ea2e458602996c measures the same visible cell across layout passes,
  preserving the10% limit, and adds a4000-velocity long fling alongside .fast.
  Run it and the identical slow-setter baselineb09f246eb38d8cf8f704 before deciding
  whether the new counter measures native travel correctly. Never weaken the
  native comparison to hide a real speed difference. The temporary phone recorder
  and its startup hook are now removed from the working tree; no phone recording
  was started. Keep its earlier frozen sources and all videos privately.
- [x] Retain the failed native-context candidate799. It passes native flick parity
  620pt versus615pt, native units, Rust, history, live-tool, exact10MB source/copy
  and10000-line code checks, but its movie flags two large backward movements.
  Exact strips2310-2314 and4353-4357 show the last answer moving upward and back
  during older-history gestures. Settled-coordinate passes do not prove motion.
  The sampled source-test RSS peaks926036KiB; this is an intermediate source,
  not the final list candidate's memory cost. Keep source-memory-799...json.
- [x] Match native flick travel in the real captured conversation. Candidatea1cd46ea2e458602996c
  passes the unchanged10% comparison using visible text: normal606pt versus native
  616pt; strong4000-velocity fling2390pt versus2446pt. Both differ by less than3%.
  The same coordinate path,637pt viewport and source depth drive both panels.
  Keep native-list-visible-flick-02.log and its passed97.04s test, fixture
  reference-flick-15d2448f4f. Test-only counters and the native reference panel
  compile solely under CARLOS_FIXTURE; Release includes neither. The raw-offset
  earlier failure remains retained, not rewritten as passing. The older slow setter
  also fails the strengthened raw-offset comparison at0.386x native travel; keep
  momentum-setter-native-baseline-02.log. Its visible-text counterpart remains
  running. The clean source de74af3e4ef7529f93d6 removes only the inactive temporary
  phone recorder/hook froma1; the native layout/controller/test code is unchanged.
  Its native/history/tool/source/code gates are queued in native-list-final-gates-01.
  Workspace cargo test passes1795 tests,12 ignored in native-list-clean-cargo-01.
- [x] Reproduce the slow-flick regression in installeda665. The owner reports no
  jerking but cannot swing far through history. Its setContentOffset(animated:false)
  call can cancel UIKit's coast. Baseline914 travels881.33pt in the initial fast
  swipe check; bounds-onlybb4 travels1163pt. The initial1.5-viewport floor is not
  the final criterion: sourcea926 shows the plain native control itself travels
  only800pt with XCUIElement.swipeDown(.fast). Replace that arbitrary absolute
  floor with the owner's stronger native comparison using identical coordinate
  paths, unchanged velocity and a10% relative tolerance. Do not tune the native
  deceleration rate or require Carlos to move faster than ordinary iOS text.
- [x] Install anchor previewa665 after its checked native/history/tool/source/code
  gates. Its signed executable SHA256 is
  e1903cf1bdab44cafd6d7e23a4833ff65b115e6fc7ca9903ad9338139d3c5c5b.
  Install succeeds at17:46Z; automatic launch fails because the owner phone is
  locked. The owner later opens it and reports the momentum regression above.
  The normal-shell daemon remains untouched, and no phone recording was started.
- [x] Compare the corrected motion on sourcea665e3dbaa349408efd9. The identical
  deep read-only UI run passes82.97s, fixture reference-motion-1e66083017; movie
  reference-anchored-motion-a665-01.mp4 is preserved. Whole-viewport phase matching
  finds only one opposite-direction candidate in that movie, at frame832. Exact
  decoded frames831/832/833 show the settings sheet beginning to open, before
  scroll gestures. The previously inspected16pt/15pt/13.8pt backward snaps do not
  recur during scrolling. The panel still traverses older content in both layouts;
  it is not frozen. Candidate generation alone is not a universal smooth-motion
  verdict, and simulator video timing is not a physical-phone refresh measurement.
  Sourcea665 also passes all native units, cargo test and the31.49s history
  failure/retry/prepend workflow with original2pt assertions. Tool/live updates,
  10MB source/exact clipboard and10000-line code remain running in anchored-check-*.
  Its separately signed phone build is queued in anchored-phone-build-01.log.
- [x] Verify native layout anchoring against the captured moving snap. Direct
  fit logs in efc5e246f0602ae3c494 and cache/offset logs in cf03f1f33ce04b81de8f
  show no custom preferred-invalidation callback. Each completed answer has one
  cache miss followed by ready hits; reparsing does not explain this reproduction.
  The long row also changes height with Flow/Gutter, which is a legitimate change
  of display mode rather than evidence by itself of a cache failure. Keep the
  numeric facts in native-layout-file-04.log and native-layout-cache-03.log.
  Broad NSLog and simulated/host unified-log queries produced no events; direct
  simulator cache-file logs work. The offset stack filter also catches ordinary
  deceleration because UIKit can report dragging and decelerating together; do
  not count every logged stack as a jerk. Restore all temporary layout/cache logs
  to the committed source before testing the correction. Sourcea665e3dbaa349408efd9
  adds only a native layout-pass reader anchor: capture visible source-cell positions
  after the pan has moved the viewport, let UIKit lay out new cell sizes, then
  cancel extra movement of an existing cell caused by changed geometry. Skip
  elastic overscroll and prevent recursive corrections. Existing idle history/tool
  anchors and bottom following keep their own roles. Record the identical deep
  gestures before installing this candidate; reference-anchored-motion-replay-01
  and recorder-01 are active. Compare exact decoded frames, not OpenCV random
  seeking, and retain the original2pt/read/clipboard assertions.
- [x] See a moving backward snap in the deep simulator movie. The actual decoded
  frames4144/4145/4146 around76.0667s in reference-deep-motion-6ffa-01.mp4 show
  the same whole transcript shifting upward16pt during a downward scroll. Three
  independent text patches match that shift at>0.999 confidence; this is not just
  a phase-correlation candidate. Inspect exact-reverse-76.png, built from exact
  FFmpeg frame indices. OpenCV random frame seeking in this variable-frame-rate
  file returned different frames, so the earlier reverse-76.png is not valid
  evidence for those timestamps. Keep both and use exact decoding for comparisons.
  Nearby candidates show15pt/13.8pt reversals. Native layout currently adds its
  own contentOffsetAdjustment to the superclass result; a20pt footer estimate
  change plus ongoing finger movement could explain these steps. This is a lead,
  not yet a proven cause. Source394910d7a104b714ad98 adds opt-in numeric layout
  logs to the same deep fixture, without changing scroll behavior. Capture the
  inherited/final offset change before choosing the fix; log reference-layout-trace-01.
- [x] Install and launch temporary phone recorder6ffab956a0f1a4b461a5 after all19
  native units pass. Signed preview6ffa-20261117T164528Z has executable SHA256
  a9223b75aa6455fa4a2d705ff4d4f06fb2891083bd3bd48b21a856c5756b5692.
  The app launched normally at16:45:36Z, without starting a recording. The owner
  has a pending Ready now/Later timing question; only launch --carlos-scroll-record
  after their reply and let them accept iOS's prompt. Retrieval helper
  device-tools/collect-phone-scroll.py can copy just the diagnostic from the exact
  iPhone to the owned Mac. Independent simulator diagnosis continues meanwhile.
- [x] Capture the actual phone motion if the simulator does not clearly show it.
  The first61.66s simulator movie (reference-motion-9a45-01.mp4) contains3320
  frames and four one-screen slow-down/fast-up cycles. Pixel phase-correlation
  locates movements but has not established a matching geometric jump. Retain
  reference-motion-trace-01.json and overview-01.png; do not call video frame
  timing an app frame-rate measurement. A new read-only deep-history UI case
  traverses six older screens then four fast returns in both Flow and Gutter.
  Source6ffab956a0f1a4b461a5 adds that case and a temporary --carlos-scroll-record
  launch mode, with no further scroll fix. It passes all19 native units. The
  temporary PhoneScrollRecording records30s of Carlos only, disables microphone
  and camera, and stores numeric display-frame/offset/pan/visible-cell facts in
  Library/Caches/ScrollMotion/<uuid>. It awaits iOS's recording consent before
  the30s timer starts. Remove the diagnostic launch hook and recorder before
  closing EP022. The first diagnostic snapshot1008 compiled with warnings because
  a zero-argument recording call did not await its callback; it was never installed.
  Use the explicit checked continuation in6ffa instead. The Mac's direct CMIO
  enumerator found no screen source and reported denied video access (status2);
  it opened no capture session and restored both screen-source flags. Do not
  change global Mac privacy settings. Phone build/sign/install02 and deep-motion
  recorder/replay01 are active, serialized by the existing Mac lock. Ask the owner
  for the brief manual recording/scroll gestures only after the recorder is ready.
- [x] Reproduce the moving jerk before changing the scroller again. The owner
  confirms that installed9a45429810146c725110 still jerks and explicitly asks Root
  to find a way to see it. The settled2pt tests and lower scroll CPU have not
  established a fix. Pause the broad matrix launcher1973481 with SIGSTOP only;
  its current code-stream fixture finishes normally. The matrix already passed
  through Ralph and prose streaming, and code-stream's own log also passes.
  Do not stop the owner's normal-shell daemon. Record the exact installed source
  with the private captured conversation, native slow/fast drag gestures, and
  simctl recordVideo on the owned simulator. Inspect the actual frames before
  another speculative fix. The bounded exact-source recorder is
  /tmp/carlos-ep022/device-tools/record-reference-motion.py; active recorder/replay
  logs are reference-motion-recorder-01.log and reference-motion-replay-01.log.
  The cold-middle-row concern is an unproven lead, not the next chosen fix.
- [x] Replace the SwiftUI LazyVStack/ScrollPosition/native-marker scroller with
  UIKit reusable UIHostingConfiguration cells and a stable-ID data source. Keep
  semantic rows, exact source readers, private tool choices and the shared client;
  delete PhoneScrollOffset. Full vertical prose/heading/table sizing also removes
  the observed truncation. The native tests retain their2pt coordinate limit.
  Preserve history01/02's36pt failed-read shifts and history03's12pt prepend shift.
  Reapply the explicit source anchor after actual layout outside gestures, and
  include the row's changing top gap when preserving the first message. Source
  c4aa20ff006b6a727aac passes history04, scroll04, the short captured-reference
  check and cargo test. Its signed app was installed and launched on iPhone18 Pro;
  the owner still reports motion stutters and jumps, so this is not a final fix.
- [x] The final installed source9a45429810146c725110 also passes real keyboard,
  Claude tools/quota, short reference, credentials, cold relaunch, token change,
  exact prompt, held approval/Stop, tool output/copy and forms in matrix native-01.
  Results remain /tmp/carlos-ep022/matrix-final-9a45429810146c725110-native-01/results.json.
  Keep the remainder running sequentially. Native bridge metrics and actual signed
  app sizes are now recorded in .agents/reports/exec022-scroll-checks.md. The owner
  has a concise optional question pending about stutters/jumps on this installed
  preview; independent checks continue without treating silence as a result.
- [x] Finish the owner's motion-stutter and jump fix. Source982f15b87f37886beb7f
  retains parsed inline documents in an NSCache (64 entries and2MiB cost hints),
  prepares up to16 upcoming entries, shares rendered blocks across cell reuse and
  removes the asynchronous22pt placeholder for already-parsed block metadata.
  Code highlighting remains off the UI thread. Defer structural transcript updates
  through the complete drag/deceleration gesture and keep only the newest pending
  update. Against the previous native build44bfcd6e902bd588621d on the same captured
  conversation, three gesture pairs use mean CPU0.802s rather than1.034s (22.4% less).
  Physical peak memory is68.5MB rather than69.0MB; gesture durations are unchanged
  at2.35s. XCTest reports no frame-rate or hitch metric here, so these numbers do
  not establish smooth physical-phone motion. The candidate passes history01 and
  scroll01 with the original coordinate/native-traffic assertions. Retain logs and
  metrics under /tmp/carlos-ep022/native-prepared-* and native-scroll-baseline-*.
  Check10000-line code,10MB source/exact Copy, syntax, full captured reference and
  native units before installing this candidate. Then complete the final sweep.
  The10000-line code check passes, but the10MB source run stalls in SwiftUI
  text layout; retain prepared-source-live-01.sample/png. PhoneEntryRow can observe
  the ready source before its parent replaces that row with independent block
  cells, briefly laying out the entire source a second time. Source
  9a45429810146c725110 keeps only the preview in that transitional answer/commentary
  cell; the parent remains the sole renderer of complete block rows. Check this
  bounded fix with the original10MB source and exact clipboard assertions. The
  corrected source passes that unchanged workflow in48.0s, native fixture
  phone-codex-9eeafe3e2ae3. Retain the failed982f run: the test could not obtain a
  screen because the main run loop stayed busy for30s. The unchanged Rust cargo
  test also passes (native-prepared-cargo-01.log). Remaining candidate checks run
  sequentially in native-source-rows-*-01.log before physical installation.
  Source9a45 also passes10000-line code36.5s, syntax/exact Copy47.9s, history
  failure/retry/prepend32.6s and tool/live-update anchors91.2s. The full captured
  reference passes56.8s in both Flow and Gutter, with no rejected/mutating reads.
  All19 native units pass with0 failures/skips. Install and launch succeed at
  16:07:51/54Z on iPhone18 Pro. Signed preview
  previews/9a45429810146c725110-20261117T160640Z/Carlos.app has executable SHA256
  0fb2975939ef081c3e37bc060290b59d13e59b810e146a58bdfc977fc4a29d6a.
  Logs: source-rows-phone-{build,sign,install}-01.log. The owner can now test the
  actual motion; do not call it confirmed smooth from settled coordinates alone.
  Continue the same-source functional sweep in final-native-matrix-01.log; seven
  exact-source passes are reused from native-source-rows-prechecked-01.json.
  The checkpoint report is
  .agents/reports/exec022-scroll-checks.md.

- [x] The owner confirms the phone reconnects to their normal-shell daemon after
  the shared catalogue copy. Read-only hello and sessions/watch accept the unchanged
  phone token and show one saved session; device/list shows its one registered
  phone. Receipt: normal-shell-phone-auth-check.json. The native session is not
  started by that check. Answered side questions: automatic local daemon behavior,
  how to run the phone listener in the normal shell, and absolute CLI/socket paths.
- [x] Frozen phone source56e36e29e767c7c9471b passes all19 native units plus keyboard,
 10000-line code, history failure/retry, tool anchors/live updates,10MB original-
  byte/clipboard, syntax, actual Claude tools and Claude quota in matrix03. The real
  reference03 XCTest passes but its temporary replay helper rejects legitimate
  entry/output and session/history reads. Fix the helper to serve only captured
  prefix rows, return an explicit unavailable error for uncaptured bodies, and
  still reject every mutation. In matrix04 the real reference then fails because
  its test assumes a fully visible small Text element after scrolling. Preserve
  ui-tests-20261117T094742 and reference-short-1306b2803d; inspect its actual video
  and screenshot before changing the position assertions. The screen remains
  populated but contains text truncation, and the owner still sees real jumps.
- [x] Full corrected terminal/native release gate04 passes, including the new
  one-header terminal Read case. Keep gate03's single Claude partial-settings timeout:
  native set_model replied success but its helper promise did not settle before8s;
  unchanged focused repeat passes0.461s and complete gate04 passes. Do not increase
  deadlines or call that earlier timeout a repaired product bug. Runtime source
 92d3a53538102e52153236eb9c0b4ba0 matches CLI/daemon/helper.

- [x] Finish the normal-shell phone reconnect after the owner reports “Daemon
  rejected this connection.” The normal-shell daemon listens on the expected LAN
  port and shared Unix socket, but device/list returns no devices. The /var/tmp
  directory inside this execution environment is private; the earlier handover
  command pointed the normal shell at a different empty catalogue. Root acknowledges
  that path error. Copy the original SQLite catalogue with sqlite3.backup into
  shared tmp/phone-preview-b7744b8/catalog/sessions.sqlite3, mode0600 in a0700 folder,
  and keep the original untouched. Verify integrity and counts: one session, one
  device. Copy the existing token privately into tmp/phone-preview-b7744b8/iphone18.token.
  Receipt: /tmp/carlos-ep022/shared-catalogue-handover.json. Tell the owner to Ctrl+C
  their foreground daemon and restart the same frozen binary with the shared
  --state-dir /home/wegel/work/perso/carlos-daemon/tmp/phone-preview-b7744b8/catalog.
  The Unix socket and192.168.3.11:33731 listener stay the same. Await their restart,
  then read device/list and authenticate with the unchanged token without starting
  a native session. Do not signal their normal-shell daemon from here.

- [x] (2026-11-17 14:43Z) Install and launch the owner's requested current app on
  iPhone18 Pro. Normal Release source56e36e29e767c7c9471b is strictly signed at
  previews/56e36e29e767c7c9471b-20261117T144140Z/Carlos.app; executableSHA256
  155fc54b826ec0aee5becc193d2c511d68af53fafecad2406157b11c4bad1fd9. Device install
  and launch both succeed. Exact logs: current-phone-{build,sign,install}-01.log.
  The old8bc sign/install logs that used those temporary names are copied to
  earlier-8bc-current-phone-{sign,install}-01.log before the new stages write them.
  Signed previews and device receipts remain separate and unchanged.
- [x] Hand over the current preview daemon to the owner's normal shell. The owner
  explicitly asks Root to stop it after asking how to run it outside this execution
  environment. This supersedes the earlier keep-PID1154145-alive rule for that one
  process. Verify its exact executable/socket/catalogue before SIGTERM; it exits
  gracefully in less than one second. Keep catalogue, device token, all binaries and
  socket directory. Receipt: /tmp/carlos-ep022/owner-daemon-shell-handover.json.
  Tell the owner to run the frozen preview carlosd from their own shell with socket
  $PWD/tmp/phone-preview-b7744b8/daemon.sock, state /var/tmp/carlos-phone-preview-b7744b8/catalog,
  CLAUDE_CONFIG_DIR=$HOME/.claude-tt and listener192.168.3.11:33731. Do not restart or
  stop their normal-shell daemon without another explicit request. Answered side
  question: branch Carlos auto-starts a local-only daemon by default; explicit
  --daemon-socket only attaches. The shared phone listener needs an explicit carlosd.

- [x] (2026-11-17 14:33Z) Check the completed phone controls and shared tool relation.
  Source833cf6a1a264ca8cf57a passes actual Claude one-header/complete-output/Full-two-row
  and exactly-one-native-prompt checks in43.9s: claude-tool-pairs-02.log,
  ui-tests-20261117T092549, phone-claude-f146f9efa59c. Its first run on73 fails at the
  settings picker before sending a prompt; the helper now waits for the session
  picker to close and for the real tools control. Source56e36e29e767c7c9471b passes
  the strengthened native keyboard/idle-caret/Done test in52.5s, including equal
  five/six-line editor heights and all original Unicode bytes:
  final-keyboard-01.log, ui-tests-20261117T092821, phone-codex-122ad6243f49.
  Root inspects the actual b604 closed-keyboard and73 Claude quota PNGs. The small
  output count is checked in c4 and remains in the current app code.
- [x] Keep old tool pairs quiet when prepending a native archive. Only results that
  existed before the prefix need a link update. Cargo08 passes the300-pair no-live-
  facts/no-reset regression and all existing tests. Commit2931ab3 adds the scoped
  relation/index/terminal fold. Commit9b60162 includes the phone quota/fold screens
  and quiet-prefix correction. A staged commit hook combined the two intended
  follow-up commits; Root corrects the combined commit description and verifies no
  code is lost. The hook runs cargo test; do not mutate the index until it exits.
- [ ] Finish the exact-source terminal package and actual mixed Claude reference.
  Release gate02 passes225 daemon tests but fails its matching-package source check
  after Rust files changed during the gate. Preserve that failed run; gate03 is
  running after the last Rust correction. The private reference loader03 fails
  before connecting because scripts/ is absent from its Python import path; fix
  that temporary loader and rerun. This is not a phone layout failure. Frozen owner
  daemon/CLI/previews remain untouched. The old owner daemon cannot supply the new
  optional tool-call relation until it is replaced at an authorized later step.

- [ ] Fix the owner's real backbuffer jumps and the reproduced large-code layout
  stall, then recheck the actual Claude reference and exact history/tool anchors.
  The b604 code-source repeat also times out; the bounded native watcher captures
  sustained99.8% CPU and73.5MiB footprint in SwiftUI graph/placement/trait updates,
  not heavy CoreText or Rust parsing. Preserve code-source-b604-01.sample/png and
  code-source-repro-b604-01.log. First bound PhoneCodeViewport's published height
  to its actual22..320pt viewport and rerun unchanged code-source assertions.
  Source9bc390ed902c073d583f still fails its unchanged UI deadline; bounding the
  viewport value alone is insufficient. Source d2e8828327390f03e2b0 gives each outer
  transcript item one VStack instead of a transparent Group while its content loads.
  Its code-source check also fails. The exact old immutable source934c188862a2d1577be1
  still passes that test in38.8s. Restore its bound ScrollPosition controller while
  retaining weak native row measurements. Sourcec4d33d61c14bf8d45580 passes all three
  unchanged checks: code-source37.3s (ui-tests-20261117T085521, phone-codex-3430561b11ac),
  history31.6s (085701, phone-codex-20a213ccebd3) and scroll54.9s (085801,
  phone-codex-1c00e7a24865). Logs are bound-position-{code-source,history,scroll}-01.log.
  This restores the large-source behavior without reintroducing SwiftUI per-row
  geometry callbacks. Check the real mixed Claude reference before declaring the
  owner's backbuffer report fixed. The compact output summary is in that source too.
- [x] Add the owner's requested weekly quota beside context, using the TUI's
  remaining-quota meaning. PhoneUsage already carries the data; PhoneActivityLine
  currently hides it for Claude with a Codex-only check and places it after the
  model. Remove that restriction and keep adjacent percentages with honest absence
  for unknown quota. The implementation now also hides expired/nonfinite values and
  matches TUI rounding/clamping and pace colors. Native Claude quota run01 receives
  the real rate_limit_event and visibly paints W60%/1.0d beside26% context; Root inspects
  its actual screen recording frame at /tmp/carlos-ep022/quota-failure-01.png. Its AX
  identifier query fails because the enclosing footer overwrites child identifiers.
  Give the footer an explicit children-containing accessibility group and rerun the
  unchanged native text/percentage/coordinate/private-draft assertions as
  claude-quota-02.log, frozen phone source73d31286f2da9e061214. That actual native
  check passes in36.3s at ui-tests-20261117T091512, phone-claude-e77426b72b49, including
  adjacent coordinates and an unsent Unicode draft. The owner explicitly supersedes
  the prototype's Codex-only quota text; do not retain that backend restriction.
- [ ] Fold each Claude execution into one expandable header without erasing original
  call/result source or merging different executions with equal labels. The owner
  reports repeated run labels; the captured real reference has23 tool calls and23
  results, with22 adjacent call/result pairs and18 equal labels. Both the terminal
  compact formatter and phone currently print a call header and a result header.
  Native fixture source keys keep the same tool identity with separate call/result
  parts. Preserve that identity rather than deduplicating strings. Add only an
  optional Tool.call entry link derived from those scoped native source keys. Keep
  original rows and output untouched. The shared client's bounded row store indexes
  actual results and folds a call only when exactly one linked result is in the
  current window. Both terminal Compact/Auto and phone Compact/Auto use this fact;
  Full retains the separate sources. Result arrival invalidates the original call.
  Late calls and history prefixes refresh an already published result's link.
  Native equal-command/late-call and shared client replace/ambiguous/partial-page
  checks cover the relation. cargo run01 catches a comparison fixture which normalizes
  row IDs but not the new linked IDs; normalizing both to their actual row positions
  preserves the full/history equality assertion, and cargo run02 passes. Run03 checks
  the late-call publisher change; the last late-call regression still needs its own
  updated run. Runs04/late-call-01 expose that the common test feed already calls
  publish(), so the test must inspect the direct reducer before that helper consumes
  its records. Run05 passes with that correct observation. Run06 additionally checks
  source-budget eviction unfolds and invalidates a retained call. Run07 adds an actual
  older-Claude-prefix case and keeps the original no-new-message/no-reset history
  invariant. All pass; no assertion is weakened. Release gate01 fails the new
  chunks_exact-to-as_chunks lint, fixed with as_chunks::<2>(). The complete corrected
  release/native gate02 is still running. Actual phone Claude call/result and terminal
  Read workflows remain pending. No frozen owner daemon or preview binary is replaced.
- [x] Shorten the phone's right-hand output count from e.g. `26 lines` to a small
  muted `26`, retaining full accessible wording and other result/status facts. The
  c4d33d61c14bf8d45580 code-source/history/scroll build checks it; final reference
  screenshots and physical installation remain part of the pending final gate.
- [x] Finish and install the owner's follow-up composer controls: keep a native-positioned blue
  caret visible/blinking while unfocused, keep it steady under Reduce Motion, remove
  the duplicate session title, shrink the mic to a plain16pt glyph and add Done above
  the software keyboard. Keep the44pt mic touch target. A small native layer uses
  UIKit's actual insertion rectangle and never changes draft/clipboard/AX text.
  Source354e053cd25cde136538 fails the blue-pixel check. Its actual screenshot shows
  a gray cursor: UIKit dims the inherited tint after a sheet closes. The bounded
  diagnostic60eb698d6d7ba73daad0 records a real2x24.33pt attached caret and the tint
  changing from blue to UIExtendedGrayColorSpace; it also fails the blue assertion.
  This is not proof of an absent cursor. Clean source3eabf70ebceda78bf8ae uses the
  explicit caret blue, removes all temporary logs and the unused-result warning,
  and avoids querying the idle caret while focused/offscreen. Its actual pixel/
  hardware/Done/private-draft check paints a blue cursor but measures no blink
  ([438,438,438,438,438,438,438]); ReduceMotionEnabled is0 in the owned simulator.
  A cancellable550ms native task changes only the idle layer's opacity and stops
  while focused/offscreen or under Reduce Motion. Source338483d5e3184163d52f passes
  both idle blue-pixel phases, native keys, Done and retained private text in53.4s;
  Root inspects its actual images. iOS renders UIBarButtonItem.done as a large blue
  pill, so replace that accessory with one plain native button on a flat44pt bar.
  Clean sourceb6045c86fecfa6a6bb65 is checking that final control and building its
  normal phone release. Its first run misses the injected Shift-Return; preserve
  plain-keyboard-controls-01.log. The unchanged complete repeat passes in57.3s,
  ui-tests-20261117T081904, phone-codex-49520e1637ef, including both blue0/438 pixel
  phases, all original native text, Done and the unsent draft. The normal release
  passes; its strict signed preview is installed at13:20UTC:
  `previews/b6045c86fecfa6a6bb65-20261117T132047Z/Carlos.app`, executableSHA256
  09002e84090a968800a96c371169aef561c3722b7b53a4d16cde0167edf51cf7.
  Install succeeds; automatic launch is refused only because the phone is locked.
  The owner explicitly asks to install input first and reports real backbuffer jumps.
  Input is installed; backbuffer stability and the large-code freeze are the next
  work. Do not silently erase the first native-key failure or call the whole EP done.
- [ ] Complete the remaining e240 functional/cost sweep at final-simulator-matrix-02.log.
  History, credentials, cold relaunch, rejected replacement, exact prompt, offered
  approval/Stop, running-tool controls, forms/questions, browser, Ralph and both
  prose/code carets pass. The10000-line code-source case then fails with an actual
 30s busy main-thread snapshot timeout; preserve e240/ui-tests-20261117T080756 and
  phone-codex-6d6189d9f6a8. The sweep stops at that failure. Capture a native busy
  stack in its next bounded reproduction after the new controls are installed.
  New composer controls need their
  own actual checks and the final source must retain all required coverage.
- [ ] Finish the owner's flat composer and the stronger native edit guard. Remove
  the pill, border and inset capsule; align monospace input with the transcript and
  retain UIKit's blinking caret and native selection. The visual-only source
  e73fabc5f617ac99e404 passes exact raw hardware keys, multiline/private text and
  the five-line bound (flat-composer-keyboard-01.log). Root inspected its actual
  image. A separate clean scroll run exposes a dropped space: the native peer gets
  `phonestream`, not `phone stream`. Native edits now update the model without
  replaying text into UIKit; only explicit Send/Restore/dictation replacements advance
  a monotonic version. A native regression checks newer text, spaces and selection
  against stale renders. The current clean build/check logs are flat-native-03,
  flat-composer-keyboard-03 and flat-composer-scroll-03 on e240e77436096c8f4454.
  All19 native tests pass without skips, including the explicit stale-render/selection
  regression. Both actual keyboard and live-scroll workflows pass on that same source:
  phone-codex-caddf734f69e and phone-codex-222fcbc0759e. The normal release phone
  build also passes. The flat progress preview is strictly signed/verified, installed
  and successfully launched at12:38UTC. Mac preview
  `previews/e240e77436096c8f4454-20261117T123752Z/Carlos.app`, executableSHA256
  bb54803de138346af3065dafe63bab3f5814a0720ffdd58ebea54688d73da8eb.
  The remaining full sweep is running at final-simulator-matrix-02.log, with completed
  exact-source native/keyboard/scroll checks retained in final-prechecked-e240.json.
- [x] Pass the clean native history failure/retry/prepend workflow on
  b757d93c53abc21091a5/ui-tests-20261117T072244, native fixture
  phone-codex-557752bcf662. Keep both axes within2pt and reach the original oldest
  message. The following scroll case passes expansion/collapse but receives the
  space-less prompt above and fails before streaming. The old-composer sweep was
  stopped after that child finished; no remaining case or overall pass is claimed.
- [x] Check earlier app/Rust source6b9031c0aba4cbf06ae8 and pass the18 native
  checks, live prose/code caret and native Stop,10000-line code fence, complete10MB
  clipboard, and exact history failure/retry/prepend workflows. Matrix04 logs retain
  each actual result and request trace. The full remaining sweep is still pending.
- [ ] Finish the native scroll/history regression before resuming the final sweep. Matrix04's
  scroll test tapped an already expanded Full-mode row and then expected expansion.
  The test now explicitly chooses Auto in the actual Settings UI; the running-tool and
  read-only reference checks do the same. Source2710dc2f61398293938c changes tests only.
  Both staged source trees have identical runtime SHA256:
  e06c979322acfc903ab915fac11c51cebc2cc2492cc27507a80df2acfbbcefbd.
  Matrix05 then reveals a real live-scroll timeout. Removing the global top anchor,
  restoring the initial bottom edge and adding a temporary touch guard each fail;
  the guard is removed. The captured native stack spends time finding lazy row
  placements, not parsing text. Source514ea35f4975b647abd2 measures native bounds
  anchors at the viewport, with no row querying an ancestor during layout. Its actual
  scroll check still stalls; its native sample and live screen are retained. A bounded
  diagnostic on d9561378ba50bd8032ff proves no repeated app offset correction precedes
  the stall: the last callbacks are interacting/decelerating with restoration off.
  Sourcec3ed87ebf02d0aa98bb8 explicitly releases the programmatic position when a
  touch scroll begins; it still times out. Source6948fda55aebdc87358d removes the
  binding and uses one-shot native jumps; it still stalls in lazy row placement,
  now without ScrollStateRequestTransform in its captured sample. The next candidate
  8bcbe1b071c29b677fe6 also removes per-row SwiftUI geometry/preference callbacks,
  using native marker rectangles only when preserving a position. Its scroll check
  passes in52.3s with the original2pt assertions and exact native Stop. History no
  longer stalls but exposes a35.67pt failed-read jump; defer its rectangle read until
  native layout settles, then rerun. All temporary diagnostic logs are removed. Keep every prior
  failure and the2pt assertions. The full `cargo test` run passes at scroll-cargo-01.log.
  Sourceb09c9d632aea980c0f46 fixes the failed-read position but still shifts32pt on
  prepend. The bounded diagnostic e918b3062af448e6b37d passes and records both a
  row jump and offset corrections on the same layout turn. The clean candidate
  b757d93c53abc21091a5 jumps by ID only when the saved native rectangle is absent;
  otherwise it corrects that measured rectangle after layout. The complete native,
  functional and simulator-cost sweep is running in final-simulator-matrix-01.log.


- [x] Build the actual native app and the narrow Rust bridge for both Apple targets.
- [x] Prove saved Keychain credentials, last host/session, rejected-token repair and
  cold restart. The physical source232 preview contains the checked token fix.
- [x] Complete native forms/questions/browser approvals, actual backend prompts,
  running-tool controls, original source Copy, streaming cursor and engine Ralph.
- [x] Prove exact old-history failure/retry/prepend coordinates and reader position.
- [x] Fix native first-character loss and prove raw Shift/Option/Return through UIKit.
  The clean keyboard13 pass records one exact multiline prompt and a bounded private
  draft; source9ce87585e585e605c9ae/ui-tests-20261117T052147, native fixture
  phone-codex-792961a82e26. It includes the large Full-mode result, but its eager
  output blocks still do unnecessary offscreen drawing; check the lazy viewport next.
- [x] Prove code colors, native file numbers and exact Markdown clipboard through
  the actual app. Syntax06 passes on9edf61c0acc0cfdf7ae7/ui-tests-20261117T051940,
  phone-codex-3cd62bea7b7d. Root inspected its image and corrected the centered short
  file box; the final check also asserts the source starts beside the gutter.
- [x] Pass the complete release terminal/native checks after the shared gutter move.
  syntax-terminal-release-02.log includes all23 Codex and10 packaged Claude cases.
- [ ] Pass the final native/keyboard/syntax/restart checks on one frozen source.
  final-candidate-source points to the current source; final-native-01 and the
  following final-keyboard/syntax/reconnect-01 logs remain pending.
- [ ] Repeat the remaining simulator workflows, inspect actual screens and the real
  captured Claude reference in Flow and Gutter. Preserve owner phone/daemon state.
- [ ] Record final simulator startup/typing/scroll/idle CPU/memory/bridge/size costs,
  frozen CLI perf-session comparisons and authored production growth.
- [ ] Build/sign/install a new immutable phone preview after the clean simulator gate.
  The owner separately requested the current progress build now. Source8bcbe1b071c29b677fe6
  is signed, strictly verified, installed and successfully launched on their specific
  iPhone18 Pro at12:11UTC. The immutable Mac preview is
  `previews/8bcbe1b071c29b677fe6-20261117T120943Z/Carlos.app`, executableSHA256
  2098adbd691688b70eabed0bd5f94ddef533a5aabfae1e027cacd7a348e94ad0.
  Both install and launch receipts report success. Existing previews and owner
  daemon/CLI remain untouched. The disclosed failed-history-read jump still needs
  repair before the final plan-approved build.
- [ ] Run Root engineering and the single end-only Fable5.2 medium whole-roadmap
  review, resolve findings, record delegated approval and archive unchanged.

### Earlier milestone notes

The notes below preserve earlier attempts, including checks that were pending at the
time. The current acceptance checklist above states what remains before approval.


- Move the terminal's existing native read-gutter split into the pure shared
  client without changing its rules. The terminal delegates to the same functions;
  the phone uses the actual prefix, never a guessed line1 offset or a display label.
  The new prefix/body test proves byte-for-byte recombination for boxed, arrow, tab
  and colon forms and leaves unmarked lines unnumbered. The combined230 client/phone
  checks and full syntax-cargo-01 workspace run pass. Run the full terminal/release
  checks after this shared formatter move before final acceptance.

- Fix the now-proven native text reentrancy. Raw HID UI07 captures the complete
  causal trace on8c1df7d6d8c41528e373/ui-tests-20261117T043052.xcresult:
  first synthetic scalar112 enters an empty view; the delegate reports length1;
  an external old1/new0 render runs before insertText returns length0. The earlier
  binding guard updated its marker in the delegate, so it treated that previous
  render as a new external change. Keep the marker only for values SwiftUI rendered
  and remove the redundant intrinsic-size invalidation. UI08 verifies this concrete
  change. Syntax UI01 also sends hone syntax rather than phone syntax and correctly
  fails; it proves no code-color UI result. Keep phone-codex-9920bd85ce52 and
  9c69953cd7a8f964a6a4/ui-tests-20261117T042737.xcresult. No test assertion or timeout
  has been weakened, and no hardware success is claimed yet.

- The real Mac grammar caller passes all15 native tests without skips on
  484a9f14cdf3e7a34a5d/tests-20261117T041937.xcresult, syntax-native-02.log.
  The standalone Apple graph compiles fancy-regex with no C regex backend. The
  first actual code-color/clipboard UI check is still building on9c69953cd7a8f964a6a4.
  A following file-output view keeps only actual native line numbers, leaves unmarked
  lines unnumbered, retains typed command/diff/error colors and parses contiguous
  plain source with one grammar state. All230 client/phone tests pass locally;
  syntax-cargo-01.log records a full workspace pass. Actual file pixels remain pending.

- Root inspected the real Ralph02 screenshots after its successful exact native
  traffic check: its engine-confirmed switch, pink status/scanner/composer, mic/Stop
  and fixed bars are correctly placed without overlap. The current full file/output,
  keyboard and final visual checks remain incomplete. No reviewer has run.

- History UI07 passes the stronger exact-coordinate checks on the pinned
  c3a71f02db8a30a1680d/ui-tests-20261117T040732.xcresult,
  phone-codex-a8ed61a3b596. The same visible message stays within2pt on both axes
  across a real native read failure, explicit retry of the original cursor and
  successful prepend; the oldest native message is reachable. UI06 is a useful red:
  its duplicate fixed-header error moves the message fromY174 to218.67. The specific
  history error now stays beside Retry rather than also resizing the fixed header.
  Full workspace cargo test passes at history-cargo-01.log. A diagnostic-free source
  rerun and the existing scroll/source regressions remain final requirements.

- Verify the first-character editor fix and raw physical Return. Raw HID UI04
  stops before the new raw x probe: the original software phone becomes hone, then
  XCTest's printable x gives honex. Five native insert callbacks are recorded, so
  unrelated SwiftUI refreshes may replace native text before its editing callback.
  Track the last binding value; apply only actual external draft changes, keeping
  native editing/IME untouched otherwise. UI05 fails before XCTest because the Mac
  dependency copy omitted a nested build/probe.rs file. That sync filter is fixed
  in da63d52 and an actual nested-source/root-build probe passes. Rerun the checked
  input after the complete dependency copy; no hardware pass is claimed.

- Require exact before/after history measurements against a held native page.
  The first three UI cases assumed where the previous row should appear instead of
  measuring before the reply; their failures remain evidence but cannot alone prove
  the size of a jump. The stronger workflow resumes an actual saved native thread,
  holds its original next page, measures X/Y, releases one real read failure, retries
  the same native cursor, then releases the page and checks both coordinates within2pt.
  The headless native scenario passes (phone-history-control-probe-03). Empty native
  reasoning rows are skipped by the actual backend, so100 native turns yield8 visible
  rows; the older30 turns yield1 actual visible row. No rows are fabricated for Swift.
  History UI04 fails in the Mac Bash3 empty-array wrapper; UI05 fails to compile
  unisolated test helpers. Both are setup failures. UI06 checks their concrete fixes.

- Check hardware Return through raw simulator HID, bypassing XCTest's character
  translation. The bounded Mac helper finds only a named Carlos simulator and sends
  Return/Shift/Option press/release events; it never links into the app. Its read-only
  API probe passes without GUI login or keyboard-setting changes. The UI keeps the
  original exact prompt/newline/five-line assertions. Raw HID UI01 fails in the same
  Bash3 wrapper; UI02 catches Swift6's test-helper actor requirement; UI03 checks those
  fixes. No end-to-end hardware pass is claimed yet.

- Ralph UI02 passes on30b5dc9007a393132442/ui-tests-20261117T034813.xcresult,
  phone-codex-9d25ae112a08. The real engine enables Ralph and sends exactly the
  configured automatic prompt; its active turn keeps Stop available after Ralph is
  turned off; the original native interrupt completes the turn. UI01 correctly saw no
  automatic prompt with the fixture's native queue unavailable. Enable the peer's real
  queue and goal methods, as the existing engine Ralph fixtures do; do not fake a phone
  phase or bypass those checks. Inspect its actual retained images before final approval.

- Wire the actual older-history call to the transcript top and verify unchanged
  reading position. PhoneHistoryModel tracks only this device's pending read; Rust
  retains the original history cursor/fence. Read failures now expose their existing
  numeric request address so an unrelated failure cannot clear another read's spinner.
  The deterministic history workflow uses130 actual native rows with sparse visible
  messages and no inference. history-ui-01.log and history-cargo-01.log are pending.

- Native input, speech and scroll checkpoint2de6b7c7100c7c369398 passes all14
  native tests without skips at tests-20261117T033135.xcresult,
  input-checkpoint-native-02.log. Full workspace cargo test passes at
  input-checkpoint-cargo-01.log. The clean voice/scroll and complete10MB-source checks
  below pass. Hardware Return remains explicitly unverified; its failing end-to-end
  assertion stays enabled. The next work adds older-history reads and preserves the
  reader's exact position before final visual and whole-EP checks.

- The10MB complete-source/clipboard regression passes after the render fix on
  3486232fd60908005d44/ui-tests-20261117T030801.xcresult,
  phone-codex-670e773c4b74, source-ui-18-clean.log. Original source length and SHA256,
  actual system clipboard bytes and Copy reset all pass. Source UI17 lost SSH before
  XCTest and is not a pass. The private request/reply assertions and waits are unchanged.

- Diagnostic-free checkpointb9aff18f402f4279cd6a compiles and passes native14
  (tests-20261117T025611), full scroll UI10 (ui-tests-20261117T025650,
  phone-codex-62300724a55e) and actual first-use denied-Speech UI07 (025839,
  phone-codex-231fd5066f25). The10MB source-copy rerun is pending; do not call that
  pass or the hardware workflow complete. Native keyboard behavior is being moved
  from shortcut registration to physical press handling after the printable x probe
  succeeds but modified Return still fails. No Mac login is required to continue.

- Full scrolling UI09 passes on6afb267a52703562c5b3,
  ui-tests-20261117T024330.xcresult, phone-codex-c4d4d7dc236f. The original2pt X/Y
  checks pass across tool expansion/collapse; two manual swipes stop following;
  native Stop completes without moving the selected older text; Latest returns to
  the answer. Keep the earlier hangs/failures. Require a clean diagnostic-free pass
  and the10MB original-source check after this fix. Native voice denial also passes;
  actual hardware Return remains incomplete.

- Voice UI06 passes the actual first-use Speech denial and original private draft
  on1509cf822de25f5d2656/ui-tests-20261117T020908.xcresult,
  phone-codex-888b6e43bbc5. The new Sendable boundary lets authorization return on
  its native queue, then the main actor receives denied/same-attempt and shows the
  persistent Settings/Dismiss message. Root inspected the actual screenshot; the
  draft stays unchanged, Send stays available, native prompts remain empty and no
  audio is recorded. Speech/scene diagnostics are removed locally; require a clean
  final pass. Hardware keyboard and the complete scrolling workflow remain pending.

- Private prompt echoes precede their own answer on source8ed2cec1d2b216555c99,
  ui-tests-20261117T012930.xcresult, phone-codex-c137582457f1. Root inspected both
  live/stopped images: the blue user band precedes the green answer; the caret blinks
  and disappears after the original native interrupt. The small Swift layout hint
  never inserts or reorders native rows, copies private text into another session,
  or changes the shared operation ledger. Queued echoes stay at the tail. Full
  workspace cargo test passes at composer-cargo-01.log. Keyboard/voice/scroll checks
  remain pending and the earlier failures remain recorded below.

- Check the native composer and on-device speech on the owned simulator.
  PhonePromptInput keeps native selection and five-line sizing; actual physical-key
  events must prove Return sends one exact prompt and Shift/Option+Return preserve
  newlines. PhoneDictation requests permissions only after a tap, requires on-device
  recognition, keeps late callbacks scoped to the current private draft and stops
  audio on exit/background. Voice UI checks deny access or show actual simulator
  unavailability without recording or sending. Keyboard UI05 now checks direct
  UIKit command admission and actual physical keys; voice UI04 checks persistent
  inline failure text; scroll UI04 checks the duplicate-margin correction. All
  three remain pending. Temporary command/offset diagnostics must be removed before
  a clean final pass. No physical microphone accuracy or completed input gate is claimed.

- (2026-11-17) Credential staging and trace-free caret pass on589f3adae575cb5d84ca:
  stream04 (ui-tests-20261117T003806), rejected-new-token repair
  (ui-tests-20261117T004433), saved-token/session relaunch (004534) and rejected
  replacement/Cancel/cold-restart (004648) all pass their exact native traffic checks.
  Local artifacts are phone-codex-527840feb882, edd76833420f,522b48d0af0e and8562385c4e48.
  All13 native tests pass without skips at tests-20261117T003735.xcresult; full workspace
  cargo test passes at native-polish-cargo-01.log. No traces remain in product code.
- Current-status polish compiles and passes all13 native checks on89ce36e8545f4ba7b2ed,
  tests-20261117T005608.xcresult, current-status-native-01.log:
  connecting/loading session labels and a muted status dot must distinguish retained
  facts from a fresh current snapshot. This does not change native state or controls.

- Protect a working saved token when the person types a rejected replacement (token-update03 passes).
  Direct code inspection shows PhoneStore.connect writes Keychain before the greeting
  accepts that token. Add a real existing-host rejection/Cancel/relaunch workflow, then
  commit the new credential only after its exact connection authenticates. Keep this
  native attempt small; no Rust outbox, persistent operation ledger or auth wizard.

- (2026-11-17) Reconnect commit4d60ba7 passes the fresh full Rust/terminal/Codex
  check at reconnect-terminal-gate-01.log, artifacts source-terminal-PYEmq3:
  all225 daemon,330 terminal and23 native contract checks pass unchanged. The empty
  replay regression and actual Safari round trip pass as recorded below.
- Finish the next visual checkpoint. Source4068648d0d3bd0edc21f compiles the opaque
  1024px icon, native named launch-background asset and scanner’s actual-turn guard;
  native test bundle tests-20261117T000834.xcresult passes with13 tests. The editable SVG and PNG
  need no build-time image dependency. A native TextRenderer is being checked to paint a green caret
  after the last streaming paragraph without adding a source character; only the
  live final block uses a clock, and Reduce Motion keeps it steady. Stream UI01 on
  5657c2c5ddec92e37f86 fails its blink-pixel check; UI03 checks selection after UI02 shows ticks without paint. It checks actual changing green pixels, unchanged
  accessibility text and original Stop/partial-answer/Copy behavior. A test-only main
  actor warning in its screenshot helper is corrected locally; verify the final run.
  Root inspected visual-polish-launch-01’s actual initial screen oned1692fe352dba5ba41a. Its low-contrast placeholders and technical setup copy are improved locally; launch02 checks those changes.

- (2026-11-17) Browser05 passes on5621087dafe487fb5108,
  ui-tests-20261116T235815.xcresult, local phone-codex-7ac042eb4735. It opens Safari,
  reads the actual page heading, returns to usable unchanged approval controls, sends
  no consent on page visit and exactly one original native accept after Allow. Stop
  stays disabled without a turn. Root inspected the return screen; no runtime-warning
  node is reported. All200 client and24 phone tests pass with the red/green empty-replay
  regression. Native tests13/13 pass on the same source, tests-20261116T235956.xcresult.
  The fresh full reconnect-terminal-gate-01 check is still running.

- (2026-11-17) Actual Claude and Grok prompt workflows pass on013def396f66309592be:
  ui-tests-20261116T235051 and235216.xcresult, local phone-claude-7a77458b4096 and
  phone-grok-e57273ab1b52. Each selects its actual backend, creates one native process,
  sends exactly phone exact prompt 🦀 once and shows the matching native answer. No user
  account/session or inference is involved. Inspect the retained screenshots before
  final UI approval. The test script separates native setup, traffic reads and assertions.

- (2026-11-17) Commit27853ca adds flat native form fields, exact integer/boolean/enum
  replies, explicit optional omission, stable sheets with retained private edits, per-card
  pending states and top-bar keyboard actions. Full cargo test passes in its commit hook;
  actual form/question/credential and native evidence is recorded below.
- Finish browser approval checks with an owned local Safari page (browser05). The app now reads
  whether an actual native turn exists so browser consent without a turn cannot offer
  enabled Stop. All23 phone tests pass after this narrow facade fact. Browser fixture01
  captured a wrong test selector variable before the correction; its frozen source must
  stay unchanged. Browser02–04 reach Safari but leave Allow disabled after return; browser05 checks the shared empty-replay fix. Opening the page
  must leave native consent unanswered, then exactly one explicit Allow response must
  reach the original native callback. No external website, account or inference is used.

- (2026-11-17, owner steering) Treat saved credentials and finished UX as hard
  acceptance criteria. The owner reports entering a token again when reopening the
  installed early app and explicitly expects a polished app at the end of EP022.
  First reproduce a real terminate/relaunch and saved-token reconnect in the simulator;
  inspect physical saved settings read-only when available. Never clear credentials on
  an ordinary disconnect or force the owner to type a saved token again. Check actual
  app screenshots and complete flows, not only whether controls exist. Do not install
  the already built intermediate5635510dc076454c0a87 phone product as the final fix.

- (2026-11-16) Close EP021b under the owner's grant, archive its completed plan
  unchanged and retain its corrected gate06/fresh frozen-pair baseline.
- (2026-11-16) Read the full phone spec, architecture, client API and Mac build notes.
  The earlier read-only Mac check passed. No app source or Mac build was created during
  EP021b; preparation notes are `/tmp/carlos-ep021b/ep022-preparation.md`.
- (2026-11-16 21:10Z) Prove the small real Swift caller before expanding the UI.
  Both Rust boundary tests and both Swift XCTest cases pass for actual greeting/reconnect
  rights, typed invalid-input/frame errors and stale socket callbacks. The SwiftUI shell
  compiles and links both generated Swift and the actual Rust static library. Evidence:
  `/tmp/carlos-ep022/first-swift-test.log`; Mac bundle3303d44c00c224ba856c,
  tests-20261116T160903.xcresult, owned simulator4C821FE4-54D8-452D-A46C-3187AAED6C61.
- (2026-11-16 21:35Z) Finish the repeatable offline build/cache path and capture the launched shell.
  Completed: project-local pinned Rust/LLVM tools, both Apple library slices,
  generated Swift/headers/XCFramework, XcodeGen project and successful real simulator
  test. The launched shell's actual screenshot is
  `target/ios-artifacts/first-shell/screen.png`. The current wrapper passes generic
  Intel/arm64 simulator app compilation and both actual native tests with all three
  Rust targets: bundle324d2d6bcadf1f77cbea, tests-20261116T163525.xcresult.
- Connect the actual app to a deterministic daemon over WebSocket; prove greeting,
  session selection/create, private prompt submission, reconnect and uncertain-write handling.
  Completed: narrow typed catalogue/session/prompt/private-result/row calls, five Rust
  tests and native socket/Keychain/model code compile. The partial-write test retains
  exact text and proves reconnect emits operation/read rather than session/send.
  Remaining: connect the actual SwiftUI controls and exercise real socket/native traces.
  Update: the actual app now connects, creates a Codex session, sends an exact Unicode
  prompt and shows its real daemon answer. The corrected full harness passed at
  `target/ios-artifacts/phone-d32e09f8acd8`, Mac bundleb7744b8a156a9c2b8287,
  ui-tests-20261116T170702.xcresult. Native unit tests pass4/4 at
  tests-20261116T170918.xcresult, including actual Keychain and bundled font checks.
  Rust boundary tests pass9/9, including original offered cards, Starting approvals,
  captured Stop turns and non-optimistic Ralph. Complete the remaining socket/control,
  reconnect/private-draft and other-backend workflows before closing this milestone.
  Update (2026-11-16 23:10Z): actual held-reply UI workflow also passes, with one original
  offered Allow once and one Stop for the captured native turn, while turn/start remains
  held. Evidence: target/ios-artifacts/phone-af93de0d4684, Mac source98d2edadc173a4374eed,
  ui-tests-20261116T180837.xcresult. A real native completion, not a local optimistic
  phase change, makes the Stop button return to Send.
- (2026-11-16 22:55Z) Build, sign and install the checked early preview on the owner’s
  iPhone18 Pro after the Mac reboot restores live device access. The exact simulator-checked
  bundle b7744b8a156a9c2b8287 builds Release for arm64; the existing development profile
  includes this phone, and strict signature/entitlement checks pass. Apple’s install receipt
  confirms app.carlos.phone. Launch was denied only because the phone locked again; the
  owner has been told to unlock/open it. A separate frozen EP021b daemon is ready on the
  private LAN, with a new private device-token file and an empty catalogue. No model call
  was made. Complete app acceptance remains below.
- Implement the specified header, transcript, composer, activity line, Latest button,
  sessions/settings sheets, native approval inputs and complete source/output reads.
  Update (2026-11-16 23:43Z): native settings/activity, both basic entry layouts, the
  whole-document pure Rust Markdown parser and original-source reader now compile.
  Source2171feece10489f25490 passes all8 native tests,0 skips, at
  tests-20261116T183512.xcresult. These include actual generated Markdown records,
  exact temporary-file prefixes, capacity preserving accepted bytes/releasing space,
  and local preference persistence. All200 shared-client and12 phone tests pass, as do
  strict phone/client lints and21 architecture tests. The source UI/full terminal gates
  are running; tool expansion, complete row grouping and other checks remain.
  Update (2026-11-17 01:02Z): private real-Claude renders pass through the actual typed
  Rust snapshot reader without a socket or credentials. Reference source6b3e10c8aacd9d1b2d01
  passes11 native tests, including10 ordinary cases and the one temporary visual check,
  with0 skips at tests-20261116T200114.xcresult. The private reference's literal tool rows
  now all measure26pt. Before the fix, short status summaries wrapped into near-zero
  remaining width and made result rows32–111pt tall. The regression test measures a real
  hosted row, checks26pt for a long command and allows real AI labels to wrap. Screenshots
  and measurements remain ignored under target/ios-artifacts/private-reference-render-03.
  The temporary visual test contains no transcript or credentials and was removed from
  normal source after staging; its scratch copy remains under /tmp/carlos-ep022.
- Complete stable scrolling, running-tool expansion, Markdown/copy, Ralph, native
  on-device dictation and honest permission/unavailable states.
  Update (2026-11-17): the saved-host/token/last-session and connection-screen fixes are
  committed as23475bb and39b4f70. The signed source2327bf4307aa3d527804 preview is
  installed on the owner's iPhone; token-preview-install.json records success. Launch
  was refused only because the phone was locked. Running/completed tool expansion is implemented
  through the existing checked output reader. It retains classified colors and an
  explicit missing-prefix notice, watches running output, closes offscreen reads and
  keeps only the old measured height while refetching. Native11 tests pass onf0b9c5056ba1d93ed13a,
  tests-20261116T220930.xcresult. The actual tools workflow passes one4569aeb392a5d716e88,
  ui-tests-20261116T221430.xcresult: chunk-a arrives before approval/Stop, chunk-b follows
  the actual Stop, and collapse/reopen preserves complete observed output. Tool commit6cf1ced
  and question commitd6045ba are checked; flat forms now pass their first full simulator
  flow. Full scrolling, remaining features and the final gates stay incomplete.
  Update (2026-11-17 01:35Z): the actual long-original-source UI workflow passes on
  source5635510dc076454c0a87, ui-tests-20261116T203335.xcresult, local
  phone-e3ad2c7d29a0. All10483884 source bytes match the system clipboard's byte count
  and SHA256 after a real Copy tap; Copied appears and returns to Copy. The fixture
  trace contains exactly the one expected native prompt and no native errors. The
  final fix targets the stable bottom row by id and follows actual visible-block
  completion, rather than trusting the estimated physical height of unmeasured lazy
  paragraphs. Running-tool expansion, full scroll checks and remaining features stay open.
- Run actual simulator workflows with daemon/native traces, memory/latency/size checks,
  Rust and terminal/native gates, then freeze exact checked artifacts.
- Complete Root's engineering review and the single end-only Fable review, resolve or
  explain every finding, record delegated approval and archive unchanged.

## Surprises & Discoveries

- 2026-11-17, Codex resume: the owner's431MB rollout produces83618724bytes
  across25 native complete-turn replies, with a47477424byte largest turn. A
 100-turn native request therefore exceeded the64MiB stdout bound; native
  Broken pipe followed Carlos closing the pipe. The metadata-only resume reply
  itself is small and already excludes turns. Probe04/05 in ignored
  tmp/codex-large-resume preserve size-only evidence from an isolated rollout copy.
  The actual owner daemon currently reports4 sessions,2 ready,1 closed,1
  unavailable and no active turns. It remains running outside the tool PID namespace.
- 2026-11-17, smaller Codex pages exposed a cached-prefix bug: readers asking for
  up to100 rows fetched another native turn even when a previous reader had just
  installed2 usable rows. Serve that available prefix first; fetch only at its
  oldest row. Both concurrent readers now share the same complete-turn read.
 15 daemon cases and both real Codex stdio/socket large-ancestor cases pass.
  Each native case retains31 turns/1711 items and reads75737895bytes overall,
  with no reply above64MiB, no item-per-RPC reads and no inference. Original
  failed checks/artifacts remain under /tmp/carlos-ep022/codex-history-*-01.

The stronger native comparison catches a regression the moderate gesture misses.
The slow setter travels618pt versus619pt native on the moderate flick but only
28% of native travel on a strong fling. Raw offsets also count revised offscreen
height estimates; following the same visible source cell instead gives an honest
comparison. The native-list candidate passes both the captured and generated
histories, and its deep movie has none of the former backward-step candidates.


The native collection alone did not remove the owner's motion stutter. Reused cells
still recreated inline parsers and briefly measured parsed blocks at22pt before
metadata arrived. Keeping parsed documents/blocks removes that repeated work;
coalescing structural changes until drag and deceleration finish prevents the
coordinator from replacing cells midway through a gesture. The measured CPU drop
is evidence of less work, not proof of a particular screen refresh rate.

- The clean live-scroll check on b757d93c53abc21091a5 receives the actual native
  prompt `phonestream`; it never reaches the streaming stage. This is input loss,
  not another lazy-stack hang. The old text-value guard can still replay an earlier
  native edit over a later space. Gate replacements by an explicit model version;
  native keystrokes update the observed text without advancing that version. All19
  e240 native tests pass, including ignored stale text/versions, retained insertion
  point, explicit clear and explicit recovery.

- Swift6.2 crashes while generating a closure reabstraction in PhoneSessionView on
  source64c749f7a5d46bf4ad6b. No UI test runs for that source. An explicit Binding
  setter closure avoids the compiler defect; no compiler setting or toolchain changes.
  Keep flat-composer-keyboard-02.log. The replacement e240 source builds and passes
  its19 native tests.

- The Mac runs out of disk while linking source5382e8b3ffb7a208ef6e; that candidate
  never reaches its UI test. A scoped cleanup removes147 inactive reproducible
  `builds/<20hex>/DerivedData` caches and increases free space from178MB to44.6GB.
  All source snapshots, test results, screenshots, device builds and signed previews
  remain. The receipt is `device-check/cache-cleanup-01.json`; the local log is
  `/tmp/carlos-ep022/mac-cache-cleanup-01.log`. The replacement source also corrects
  the compiler's GeometryProxy capture warning before testing.

- Real scrolling can stall inside SwiftUI's lazy row placement. Source882f474f0d4243e91591
  reproduces a sustained99.7% app CPU sample in `ScrollStateRequestTransform.findClosestSubview`;
  `target/ios-artifacts/scroll-anchor-hang-01.sample` retains that stack. Sources
  2ca4332c976d1ab8d14e and d49defef3759e7358d03 still time out after the initial
  bottom-edge and temporary touch-guard changes. No passing result is claimed for them.

Update2026-11-17 Root, final scroll checks: matrix04 fails at expansion because the
previous workflow left Full mode selected; its saved AX tree shows the tapped row is
now Collapsed. Matrix05 explicitly selects Auto and passes expansion/collapse, then
hits the unchanged180s deadline during swipes while a native reply is live. Retain
2710dc2f61398293938c/ui-tests-20261117T060039 and phone-codex-661b78991fa7.
Do not call this a proven100%-CPU rendering loop: the subsequent process watcher never
captures a sustained hot process. A repeat instead catches a20pt expansion jump
(Y702→722) on the same source, ui-tests-20261117T060726, phone-codex-2a67ea07bb2f.
Its post-test Home screen is normal teardown, not evidence of a crash.

The history checkpoint added a global .top scroll-position anchor on top of explicit
per-action anchors. Test removing that default while keeping the history's measured
row/id/point correction unchanged. The scroll/history pair is pending on882f474f0d4243e91591;
retain anchor-scroll-01 and anchor-history-01 logs. If the jump remains, reuse the
existing measured-row adapter for the tapped tool rather than add another scroll engine.


Update2026-11-17 Root: final-native01 passes18/18, no skips, on98ebb887daa478d26ce8/
tests-20261117T052820. Its real Swift/Rust measurements are26.70–28.14ms per1000
bounded Markdown reads and77.70–78.94ms per500 Rust code lines (19000UTF8bytes),
after grammar warmup. The unit process peaks at46.6MB; this is not app memory.
Final-keyboard01 passes with Full explicitly selected and real10000-line output loaded:
98ebb887daa478d26ce8/ui-tests-20261117T052901, phone-codex-e3ed151a5c84,46.93s.
The earlier eager chunk case passes but takes111.87s and its live sample shows CoreText
drawing offscreen blocks. Lazy output blocks keep only nearby labels drawn.

The strengthened file-X assertion rejects final-syntax01: the source starts90.33pt
from its viewport edge instead of beside the gutter. An explicit viewport minimum width
corrects that, and final-syntax02 passes on776494ab68c6866221ca/ui-tests-20261117T053614,
phone-codex-3066bdc62224. No assertion was removed. Native checks pass on this source too.
The same candidate's actual restart/Reopen check passes:
776494ab68c6866221ca/ui-tests-20261117T053803, phone-codex-f397b0c77fd8. Its
native evidence proves one original send, two actual resumes (operator recovery and
explicit Reopen), both private texts retained, no private draft in an independent
reader's snapshot, and no right for that reader to inspect the original operation.

A direct call-path check shows Reopen alone cannot resume a failed subscription.
The phone now records its private Reopen operation key and subscribes only after an
actual accepted reply, with controls busy until that reply. Rejected/uncertain replies
retain private text. Move explicit native controls into PhoneHostControls.swift so the
host model stays comfortably below400 lines. Navigation stops microphone capture and
dismisses the editor before showing another input sheet.

Apply the same bounded text viewport to Markdown code fences, which otherwise retain
the exact oversized-label hazard proved by the output sample. Add a real10000-line
native code-fence/complete-clipboard check; keep the existing10MB-source check intact.
These new shared-view changes and the stronger final matrix remain pending.


Update2026-11-17 Root: keyboard12 still freezes at100% CPU before Send on
9edf61c0acc0cfdf7ae7/ui-tests-20261117T051046. The live sample proves CoreText
spends its time measuring one10000-line label; bounding only its scroll box is
insufficient. Keep target/ios-artifacts/keyboard-12-hang.sample and keyboard-12-live.png.
Split normalized display output into at most64-line text blocks off the UI thread,
keeping colors/native numbers and the original read unchanged. The small scroll box
retains every block and uses natural height for short output. Keyboard13 checks
this exact new source; its result is pending. The later final keyboard check explicitly
sets Full and waits for history-output0 before typing, so it cannot pass accidentally
under a previous test's collapsed global preference. Reconnect03 hit the same early
Full-output freeze before reaching the actual restart; it proves nothing new about
the catalog-based Reopen fix. Retain9edf61c0acc0cfdf7ae7/ui-tests-20261117T051535.

Add repeatable native XCTest measurements for saved launch, private typing, scrolling,
idle CPU/resident pages and generated Rust calls. These remain test-only; no runtime
observer or product test hook is added. Grammar licenses now include the pinned Sublime
default set and its separate Rust syntax notice, in addition to syntect's MIT text.


Update2026-11-17 Root, clean simulator checks: the release terminal/native gate02
passes, including all23 public Codex and10 packaged Claude cases. Keyboard11
(source58bb8a238b6c26ecc79d/ui-tests-20261117T045706, phone-codex-74dd09d47c7b)
sends exactly one original multiline prompt, but the actual transcript becomes blank:
Full mode opens the native fixture's10000-line output and the lazy list estimates
a853723pt content height. This is an app failure, not a keyboard-test false alarm.
Bound the output viewport to320pt while keeping every checked byte reachable through
native horizontal/vertical scroll. Short outputs retain their measured natural height.

Syntax04's AX tree proves the file read exists with its actual path and native
numbers. Its output is a ScrollView, not an Other element, and sits above the viewport
while the composer keeps keyboard focus after Settings. Check the actual element
type, deliberately reveal it, and dismiss composer focus before opening navigation
sheets. No file-color UI pass has been claimed.

Reconnect02 reaches actual uncertainty with both original texts retained, but its
Reopen button stays absent: the catalog reports the actual stopped session while
the cached transcript still says ready. Read lifecycle availability from the current
catalog first. Its repeated downstream assertions eventually hit the unchanged180s
UI deadline; retain the failed result52c94c22819b7865eef6/ui-tests-20261117T050136.
The new fixes remain pending exact clean checks. Temporary input traces are removed.


History UI01 and02 load the complete older native page but fail the retained top-row
check: anchor30 moves toY257.67 and252.33 instead of staying belowY177. Keep
phone-codex-f3e481ff2711/ef8a76e18b9f25ec316a/ui-tests-20261117T033932.xcresult
andphone-codex-3bf4f3f0985a/3b35002f3358b9a215d3/ui-tests-20261117T034519.xcresult.
Root inspected the actual last recording frame: the newly prepended user0 replaces
user30 at the top. Identity-based scrolling and eager bounded prefix headers alone
are insufficient. Finish the gesture before requesting a page and retain the visible
row's measured content origin across the prepend. The correction has one stable row,
clears on the next user gesture, and avoids a clock, full height cache or invented rows.
History UI03 checks this change; it is not yet a pass.

iPad08 (keyboard-ipad-ui-08-enter.log) inserts the literal copyright character after
phone when XCTest sends modified XCUIKeyboardKey.enter. The installed XCTest constants
are CR13 for Return and ETX3 for Enter; the Mac uses the Canadian keyboard layout.
An ordinary x succeeds, but neither special key proves Return delivery. Restore the
canonical Return constant in the test and retain the failure rather than changing
product behavior to accept a wrong character. No Mac keyboard/desktop settings changed.

The printable physical x probe succeeds on the iPad, then Shift+Return still fails;
keyboard-ipad-ui-05.log, source27843e22fb418cf78534/ui-tests-20261117T025317.xcresult,
phone-codex-db55d0449397. Thus general event delivery works, and missing GUI login
is not a demonstrated blocker. The direct pressesBegan/Ended implementation also
passes its native editing test but Return remains unobserved in iPad07. Keep that
failure: phone-codex-53f46ef69154, keyboard-ipad-ui-07.log. iPad08 tries the physical
Enter key constant with the same native press handler, printable-key probe, exact
multiline prompt, five-line height and private unsent-draft checks. It supports both
main Return/Enter and keypad Enter by their actual HID codes. Do not claim main-key
synthesis or any hardware success unless the end-to-end check passes.

Source UI17 (phone-codex-b78e3c4b76d4) loses SSH after framework generation; no result
bundle is logged. A following queued keyboard06 stops in the new source-selection
guard although a fresh shell has no override set. Editing a shell script while its
process waits before parsing the remaining file can corrupt that queued invocation;
this is the likely setup cause, not a product result. Move the Mac lock into already
parsed build/toolchain functions. Fresh invalid snapshot ids are rejected before
remote work; pinned-framework-01 correctly rebuilds the existing b9aff18f402f4279cd6a
snapshot. CARLOS_IOS_SOURCE now allows exact frozen-source reruns while Root continues
independent work. No Mac orphan compiler remained before the fresh source/UI reruns.


The Mac has no logged-in GUI session: /dev/console belongs to root and gui/501
returns125. Opening only the owned Simulator window fails with OSLaunchdError125
(domain does not support this action); no desktop/account settings were changed.
Root incorrectly started iPad04 despite that failed setup; it is an unchanged-environment
retry, not new evidence for the GUI hypothesis. iPad05 first sends an ordinary physical
x and requires phonex, then deletes x through the existing text API before the original
Return checks. This separates general hardware event delivery from Return command
routing. Never infer that the editor is wrong from a test channel that cannot type a
printable key. Keep the native exact prompt assertion unchanged.


UI08 also samples100%CPU after the live answer appears; retaining full measurements
outside view state alone does not resolve layout. Keep native text-view updates
idempotent: configure typing attributes once and reset them only when external draft
text actually changes, rather than resetting the native editor on every SwiftUI pass.
Keep Return command objects stable across key-down/up queries. The inline Markdown
renderer also nests a lazy stack inside each lazy transcript row. Inline previews are
bounded, and long answers already flatten into independent root rows, so use an
ordinary VStack for those inline blocks. This removes a second lazy-layout estimator
from each short row instead of adding a new scrolling framework. Repeat the original
scrolling contract and complete-source check against the corrected exact source.


Scroll UI07 also reaches the unchanged180second timeout on8d8f2f094e8fe5e82b22,
ui-tests-20261117T022521.xcresult, phone-codex-b5a3109528fc. Removing the dynamic
size anchor alone is insufficient. UI08 keeps raw geometry in a small non-observable
measurement holder and publishes only the40/260pt Boolean thresholds to SwiftUI;
full offsets must not rebuild the lazy list on every layout. It also pauses following
from the first tracking touch, not only after dragging starts. Explicit bottom requests
still follow actual transcript/formatter changes and Send/approval/Latest. No viewport
callback writes another bottom target. Recheck the same workflow and long-source case.
The iPad typed-key overload is available in the installed SDK but still needs its
actual-result check. Literal prompt editing now disables automatic word/case/quote/dash
substitutions; exact code and shell text must remain what the person entered. The
next iPad03 repeats the unchanged physical Return assertions with that native policy.


The first owned-iPad test b609aeb1c89b65d772ec/ui-tests-20261117T022116.xcresult,
phone-codex-1d324186b832, still fails Shift+Return, while the direct action test
passes. Unlike the iPhone run, its native trace proves a physical Shift press
(code225, flags131072) reaches the editor, but no Return press/action follows.
Apple’s current Swift API has separate typed-key and String overloads; the test
had passed XCUIKeyboardKey.return.rawValue to the String overload. The next iPad
run uses XCUIKeyboardKey.return directly, keeping every draft/height/native-send
assertion. No further product command change accompanies this test correction.
Do not claim hardware keys complete until the real app path passes.


Scroll UI06 on6c46856f0b7c1e831088/ui-tests-20261117T021413.xcresult hangs the actual
app after Send with the keyboard open. Carlos PID68528 stays at100%CPU; the visible
working timer remains at0:01. A3second native sample puts the main thread in repeated
SwiftUI lazy-list placement/graph updates (target/ios-artifacts/scroll-06-hang.sample).
This run cannot be explained as an accessibility-only failure. Remove the competing
size-change bottom anchor; follow explicit transcript/formatter changes and explicit
Send/approval/Latest, with a separate follow only when viewport height changes. Do
not toggle following from every non-user content-size change. Keep the exact visible
position/native control checks and repeat the long-source workflow afterward. The
existing test retains its180second cap; any forced fixture-only cleanup is recorded
as interruption/failure, never a pass. Owner daemon and physical app stay untouched.


Scroll UI05 passes expansion/collapse and both X/Y assertions on7c494f222c117df590a3,
ui-tests-20261117T020555.xcresult, phone-codex-1df25198cfe3, but cannot query Latest.
The native log proves idle bottom distances776.33 and1325.33pt. Root inspected the
actual video at33seconds: Latest is visibly present, so the issue is its inherited
accessibility id. Scope transcript’s id to the ScrollView itself before adding the
overlay, rather than to the outer GeometryReader which also wraps Latest. UI06 keeps
the original button id query and every native/pixel assertion. The video is variable
rate (2007frames/48.1seconds); selecting frame50 shows the initial Home screen, not
the final failure. Use timestamps for future video checks. Earlier selected-frame
Home images cannot establish an app crash. The separate read-only live Home shot
was also after the voice assertion, so it does not alone prove its termination cause.


Scroll UI04 passes its expansion and collapse Y checks onf6327652a1f0a59ad7be,
ui-tests-20261117T015630.xcresult, phone-codex-120e82f143ce, then fails to find Latest
after one swipe. The exported expansion image also reveals a horizontal shift:
the outer16pt padding sits outside the scroll-target coordinates. Put that margin
inside each row and keep the target stack full width; add an X-coordinate assertion.
Two deliberate swipes must pass the260pt Latest threshold even with a short viewport;
retain the same waiting time, native Stop and stationary-reading assertions. UI05
also logs actual idle distance. No full scroll pass is claimed yet.

Voice UI05 reaches Start then inactive, with no permission-completion log before the
app vanishes; source3f896234ea850d77ca63/ui-tests-20261117T015941.xcresult,
phone-codex-2ecc8536a363. The read-only live image is the Home screen, not a hidden
error banner. No Carlos crash report or XCResult console log is available. The SDK
states the authorization callback does not use the recognizer’s main operation
queue. A non-Sendable callback created inside the main actor can inherit that actor
and fail when invoked on another queue. Explicitly mark native speech/audio callbacks
Sendable and move only copied text/flags to the main actor. UI06 checks this narrow
thread-boundary fix; the exact termination cause remains an inference until checked.


Keyboard UI05 on91c04054ec3c3716d3fe/ui-tests-20261117T015338.xcresult,
phone-codex-ac527f767a91, passes the direct UIKit action test but still fails the
physical Shift+Return assertion. The trace proves direct actions execute in the
unit process; the focused app still queries commands without invoking them. The
explicit canPerformAction override alone does not fix routing. UI06 records only
hardware key codes/modifier flags and whether a key is CR/LF, plus inherited Return
commands; it never logs prompt contents or credentials. Do not replace the real
physical-key check with the passing direct-selector test. Voice UI04 on61e499ebef31e88ec924,
ui-tests-20261117T015013.xcresult, phone-codex-b256769f586a, still fails to find the
persistent inline error after denial. Voice UI05 traces start, authorization identity,
cancellation and scene/view changes before another product fix. The earlier alert
hypothesis is not proven. No intermediate reviewer has been invoked.


Scroll UI03 still moves the header by6pt on a909ffdb63918c22e7b6,
ui-tests-20261117T014801.xcresult, phone-codex-e840a6b842a1. The native trace proves
contentOffset5312.67 and top inset0, so adding/removing the inset cannot explain it.
The lazy stack also has6pt of outer top padding while its first item already owns
6pt. Remove the duplicate outer margin so the scroll target coordinates and measured
content coordinates share one origin; UI04 keeps the same2pt assertions. Retain
native-input-scroll-trace-03.txt. Also fence speech’s final draft callback by the
captured session directly, because a SwiftUI onChange callback alone can run after
a native recognition callback has already queued on the main actor.


Keyboard UI04 still fails the first Shift+Return on a909ffdb63918c22e7b6,
ui-tests-20261117T014602.xcresult, phone-codex-ad2d31c08f7a. The native trace shows
keyCommands queried twice with focused=true and marked=false, but neither custom
action executes. Explicitly admit only the two editor actions in canPerformAction;
keep all other UIKit admission unchanged. UI05 includes both the direct UIKit test
and the original physical-key app test. The private diagnostic contains no text or
tokens: target/ios-artifacts/native-input-trace-04.txt. Remove temporary keyboard and
scroll diagnostics and require a clean rerun after the fix. The first unit invocation
also captured the old optional-selector test compile failure on6d8919cbabe850661535,
tests-20261117T014452.xcresult; its fixed selector is checked in the next source.


Scroll UI02 reduces the header jump from840 to6pt, but still fails the original2pt
tolerance on108da9486b4baa513ab2/ui-tests-20261117T014047.xcresult,
phone-codex-b3d500dc66a2. UI03 uses the raw measured content offset instead of
adding the content inset; temporary offset traces will identify the exact coordinate
values and must be removed before a final pass. Keyboard UI03 still fails its first
modified Return on eaf2c86cd7b1f67de684/ui-tests-20261117T013801.xcresult,
phone-codex-a8b14d49e7c2. Add a real UIKit command-admission test and temporary
command-state traces; do not guess another cause or weaken the per-key assertion.
Voice UI02 also fails to present the app alert after the native denial. Keep the error
beside the draft instead, with Settings and Dismiss actions. Voice UI03 cannot test
that behavior because Root’s new UIKit unit test incorrectly passed an optional
selector to canPerformAction. Unwrap the actual SDK selector; retain the failed
compile at9c3e4ed24baa04694d50/voice-ui-03.log, phone-codex-f8a660072fac.
Voice UI04 will test the corrected build. No hardware speech accuracy is claimed.


Scroll UI01 proves a real jump: expanding the last tool moves its header from
701.67 to -138.33pt. Preserve sourcee6458813f8219a9b6e43,
ui-tests-20261117T013148.xcresult, phone-codex-55fdd98ca027. The phone was still
pinning the bottom while a person deliberately expanded output. Before that tap,
replace the bottom/id scroll target with the measured absolute offset and stop
following; resume following when within40pt or after Latest/Send/approval. UI02
keeps the original2pt screen-position tolerance and exact native checks.


Keyboard UI01 (4049cfc7b213d0c02604/ui-tests-20261117T011903.xcresult,
phone-codex-f2d0c49e16ed) fails before sending: custom modified-Return edits appeared to lose the newline and following word. Explicitly notifying the native
text-view delegate after programmatic insertion does not fix UI02: its first
Shift+Return assertion still reads only phone. Root also found the test used a raw
carriage return instead of XCTest’s physical Return key constant. UI03 uses the
SDK constant and retains each per-key assertion. The cause is not yet proven. Voice UI01 (9409def3dcd22f9d877b,
ui-tests-20261117T012316.xcresult, phone-codex-be6cdd5c287a) reaches the actual
Speech permission dialog and denies it, but never shows the app error: the dialog
can make the app inactive, and the initial code canceled authorizing attempts in
that state. This code path is a concrete risk, but the continuing UI02 failure means
it is not yet a proven full explanation. Keep authorization
alive through inactive permission dialogs; backgrounding still cancels it. Voice
UI02 resets privacy rights only for the synthetic fixture bundle and rechecks the
actual denial. No recording or native prompt is permitted in this workflow.


The token replacement workflow reproduces the existing-host failure on4d683461d007481a9358,
ui-tests-20261117T002718.xcresult, local phone-codex-67093b172e86: after a rejected edit
and Cancel, the old connection cannot send (the assertion names this exact behavior).
The current fix holds a new token only in a small native connection attempt, saves it
and its configuration only after that exact authenticated greeting, and fences old UI
callbacks by attempt identity. A blank existing-token edit does not rewrite Keychain.
Cancelling the sheet resumes the previous saved connection only while active/unlocked;
new rejected hosts are not persisted. Token-update UI02 passes on34bdad3e0b4ed7c84eac (ui-tests-20261117T003504);
the final guarded UI03 and native13 tests pass on589f3adae575cb5d84ca. This changes no protocol, Rust operation store or owner daemon.

Streaming UI03 passes on53c248595f1e2a1e6e5d, ui-tests-20261117T002438.xcresult,
local phone-codex-25f123fef314. Actual green-pixel counts change from267 to1239; the
render trace now shows paint on each clock phase with a6x18 caret after native glyph
bounds. Disabling selection only while streaming enables TextRenderer. Completed text
remains selectable, with exact original accessibility text and byte-counted Copy after
native Stop. Temporary traces are removed; the final trace-free stream04 passes on589f3adae575cb5d84ca.
The stream image also places a private local echo below its native answer; check the
fixture’s user-item events and the existing shared operation anchor before changing UI
order. Do not reorder rows heuristically or invent a confirmed native user entry.


Streaming UI01 fails its actual pixel check: all seven samples contain265 green pixels
from static content and no visible blinking caret. UI02 adds geometry-only traces and
also fails. Its log shows the periodic clock changing phase, but no renderer paint log.
Root is testing native text selection as the cause: the active answer now disables
selection while using TextRenderer; completed text remains selectable. UI03 checks that
single behavior change and logs draw entry/geometry without logging source text. Do not
add an animatable workaround or change the pixel assertion before reading these traces.
The screenshot helper now stays on the main actor and holds its RGBA buffer for the
whole CoreGraphics draw. The final caret change must remove temporary logs.


Forms UI08 passes on60f84be7db4c644d6cd8, ui-tests-20261116T233112.xcresult,
local phone-788b8adfbd8d. Root inspected the keyboard-open image: Done is in the top
bar and every bottom action stays clear. The form keyboard focus/defocus warnings are
gone from that run; one warning remains during the old connection keyboard toolbar.
The connection placement fix is checked by keyboard-credentials-01, still running.
A direct read also found that an integer exceeding u64 could fall through to f64 and
silently round. Integer inputs now accept exact i64/u64 digits and reject overflow,
fractional/exponent syntax instead of changing the value. All23 phone tests pass,
including >u64, below i64 and beyond-safe-float decimal inputs; the daemon still checks
native schema/range limits. The shared form-reader code remains unchanged since the
full terminal gate began; only this phone-local numeric encoder changed afterward.


Questions UI05 passes onb4d1cb4487bc103e6473, ui-tests-20261116T232528.xcresult,
local phone-50f78614b6b5. The sheet closes/reopens with its selected choice and exact
private note, then sends that original native response once. Forms UI07 passes on
bf5931634d358dedfdc9, ui-tests-20261116T232751.xcresult, local phone-248c716b6411.
All12 native tests pass without skips on that same source, tests-20261116T232946.xcresult.
Root inspected all three form screens. The keyboard-open image catches iOS26's floating
keyboard Done overlapping the bottom action bar. Root moved Done to the top bar and
added an actual frame check that it stays clear of Submit/Deny/Cancel; form UI08 checks
that polish. The connection editor gets the same top-bar placement and its own actual
keyboard screen/frame check. Neither change alters keyboard entry, saved secrets or
native approval rights. The existing Invalid frame dimension runtime warning remains
under investigation; its captured system log backtrace has SwiftUICore, SwiftUI and
UIKit frames, with no app frame. Do not silently call it fixed from a test PASS alone.


The first full form terminal gate fails one Claude settings disk-error subcase after
225 daemon tests: the native fixture records a successful set_model response, but its
compiled SDK helper does not settle that original request before the operation deadline.
The isolated unchanged case passes in0.720s (forms-settings-recheck-01.log). Preserve
forms-terminal-gate-01.log and source-terminal-6csLYG; repeat the full gate without
changing deadlines. No phone/form mutation or form reader was involved in this case.
The second gate is forms-terminal-gate-02.log and now passes fully, artifacts source-terminal-ljZ4xx.


The form focus trace on source23a7159681c53e3abd3e shows focus=true, then false
and field disappear/reappear when the keyboard opens. The sheet was attached to a
lazy transcript row, which SwiftUI can discard as the keyboard shrinks the visible
transcript. Root moved the sheet and retained per-card drafts to PhoneSessionView;
transcript cards now ask that stable parent to open them. Forms UI06 checks this
change before any further keyboard adapter changes. UI05 remains failed, not evidence
of a working numeric editor. UI06 passes onbfe41a4a308d14510fdb,
ui-tests-20261116T232246.xcresult, private local phone-8273bea56ea8. It checks invalid10,
backend rejection, retained choices, correction5 and exactly one typed native answer
with false, fr and omitted optional. Root inspected its actual screen: all fields fit,
choices remain clearly selected, and fixed Submit/Deny/Cancel controls stay visible.
Temporary focus-only logs were removed after proving the cause. The final form check
also captures keyboard and rejected-answer screens; question UI05 checks close/reopen
at the stable parent. No UIKit field replacement or wire change was necessary.


Form UI01 fails before a submission: the count field is above the lazy transcript's
visible area with the composer keyboard open, and XCTest reports an infinite frame
when it attempts the native scroll-to-visible action. Keep that failure at
forms-ui-01.log/source020d8e701c12b70b9abf. Move structured fields to a large native
sheet, with a regular scroll view and fixed Submit/error controls above the keyboard.
PhoneQuestionDraft retains the original Rust editor and private edits outside the
sheet, so Close or swipe-dismiss does not clear them. The parent card offers Review
form/Answer questions; its complete native caption remains under Request details in
the editor. Backend decisions stay explicit and only native resolution dismisses a
submitted request. Forms UI02 (source3188d5fb0fdfc5d15db1) and questions UI03 are
running. The latter now closes/reopens the editor and checks the exact retained note.
Native12 tests passed on3faf19d6013f3f06e206 at tests-20261116T225237.xcresult before
this sheet change. All23 phone tests, existing1096 core tests and the new shared form
description cases pass; full cargo test passes at forms-cargo-test-01.log.

Questions UI02 passes on19fcce42a4d264823b4a at
ui-tests-20261116T223614.xcresult, local phone-b27be74f8e41. It selects the offered value,
enters a private Unicode note and submits exactly one original native callback reply;
only the native resolution removes the card. UI01 exposed a real binding mistake:
structured questions need not offer an empty Allow-once decision. Input availability
comes from the captured Fields variant, independently of decision buttons. Remove that
false requirement; keep all host/snapshot/card/field checks. The corrected unit fixture
also omits Once. All18 Rust cases pass; native tests pass12/12,0 skips at
tests-20261116T223749.xcresult. UniFFI's generated errorDescription contains Swift enum
syntax, so PhoneFailureMessage now extracts the original typed message consistently
in app error banners. The original failure retained the private note and sent no reply.

Structured questions use existing protocol InputField/FieldAnswers data. The phone
binding captures the original fields and host/card in an opaque PhoneQuestions object;
Swift receives labels and selected positions, while Rust maps positions back to the
original typed values. It rejects changed/retired/foreign/offline cards and oversized
private text before allocating an operation. All18 Rust phone tests pass, including6
new question/browser cases, and strict phone clippy passes. Native app/tests compile
on16e4ba16c5cc73d94262, tests-20261116T223034.xcresult. The actual questions UI workflow
is running at questions-ui-01.log. Forms still need their flat native editor; the
temporary terminal-only form caption is explicitly unfinished, not accepted EP output.
The full terminal gate plus isolated Codex contract is running at
questions-terminal-gate-01.log. The wrapper differs from scripts/test.sh only in its
explicit checkout and owned /tmp artifact root, retaining all assertions and deadlines.

Running-tool workflow02 passes on e4569aeb392a5d716e88 at
ui-tests-20261116T221430.xcresult, local phone-ab5a7afca629. It opens a running tool,
reads chunk-a, answers the offered approval, sends the original Stop, receives the
fixture's native chunk-b, and preserves/reopens the row's output. Exactly one held
prompt, one answer and one interrupt reach the native peer. Workflow01 queried the
raw shell command, but the engine publishes the existing descriptive label Read log;
its long wait reached the unchanged native command deadline. Correct the test query,
not the deadline or the published tool facts. Native tests pass12/12,0 skips at
tests-20261116T221711.xcresult; cargo test passes at tool-cargo-test.log. Root inspected
the actual expanded/stopped screenshots. Remaining visual work includes the old
decorative glyph in some tool labels and the approval card's generic terminal caption;
the complete scroll-anchor, syntax, structured approval and dictation work remains.

The saved-connection/render preview2327bf4307aa3d527804 was built Release, signed with
the existing same-team/keychain rights and installed on the iPhone18 Pro. Signed app:
/Users/wegel/carlos-ios-build/previews/2327bf4307aa3d527804-20261117T031554Z/Carlos.app.
Executable SHA256:02c77bd75edb2255373568fb0dcfcfde0075582d36f06b712e89be4e83e48221;
unsigned executable5590168 bytes. The install receipt is token-preview-install.json
under the owned Mac device-check directory. Launch was denied because the phone is
locked; no unlock request or human gate is needed for continuing the remaining EP.
This installed preview includes the token/render fixes, not the newer running-tool
work. The owner daemon and CLI stay untouched.

The connection editor now waits for the actual authenticated greeting before opening
Sessions. Credentials run01 and relaunch04 exposed that the keyboard could cover the
old Form-row Connect action; the captured hierarchy placed its center under the
keyboard toolbar. A fixed safe-area action bar corrects that workflow. Credentials02
and relaunch05 both pass on2327bf4307aa3d527804 at
ui-tests-20261116T220323.xcresult and ui-tests-20261116T220455.xcresult, local
phone-34ae0a53b15c and phone-b2fd77a9aae3. They check a rejected token remains editable,
correcting it authenticates, saved-token reuse, edited-address protection, background/
relaunch and last-session restoration, with no duplicate native creation or prompts.
Root inspected the actual rejected/accepted screenshots. A new helper assertion also
requires Connect above the keyboard. Native tests pass11/11 on the preceding e5d044
source, tests-20261116T220237.xcresult; cargo test passes at token-cargo-test.log.
The connection form has themed rows, explicit keyboard Next/Go/Done, Cancel for sheets
and a15-second greeting deadline. These checks do not close the overall EP.

The physical phone's saved preferences were copied read-only with devicectl. They
contain two host configurations and no token: the first has a mistyped out-of-range
port337310, and the second has the working33731 address. The early app always chooses
the first host on launch. Do not erase either configuration or Keychain item. Remember
the chosen host, migrate older preferences to the last configured host and reject
out-of-range ports before saving. Also remember each host's last session and subscribe
after its authenticated greeting, without creating a session or resending a prompt.

The actual relaunch baseline96a88f2e20ec4e9681e5 fails specifically because Connect in
the saved connection editor is disabled with an empty token. The host and Keychain token
do survive termination and authenticate. Source53806687286591e7c697 passes the real
foreground/relaunch/reconnect workflow at ui-tests-20261116T215303.xcresult; local
phone-dee3d8861fb3. It proves token reuse for the same saved address, requires a new
token for an edited address and records no prompt or extra native session. Later
host/session-choice changes are being checked by token-relaunch-03 and native tests.
The exact physical Keychain contents were never read or printed.

Update: token-relaunch-03 passes on372047ff1eba09c65c43, local phone-62330516db28,
ui-tests-20261116T215520.xcresult. It also checks that reopening restores the last
session's actual title. Native tests pass11/11,0 skips onae11b9f0c683d96c0c8c at
tests-20261116T215431.xcresult, including invalid ports and saved-host migration/choice.
Root inspected the actual token-editor screenshot: the saved-token hint is visible;
the current editor still needs the final keyboard/navigation/theme pass. The earlier
read-only Claude Flow/Gutter screenshots were also inspected: fixed controls now paint
in both layouts. The physical phone remains on the early preview until the next checked
bundle is signed and installed.

The actual-app read-only replay is more reliable than the temporary in-process window
helper. Do not run more variants of that helper. A private replay serves only hello,
sessions/watch and session/subscribe from the captured Claude snapshot, accepts a fresh
synthetic token, rejects every mutation and never connects to the owner's daemon.
The full76-row and short5-row cases traverse the actual connection/session/settings UI
and both layouts. Short replay969dfdca38 and full replay427397ec4a pass their geometry
and interaction checks, but the short Gutter screenshot reveals that valid/hittable
fixed controls can still lose their painted contents. Opaque fixed bars above the scroll
view correct this; reference-short-ad77e24c37 passes an additional exact title/draft
pixel comparison across Flow and Gutter. Its source is0fd6685d4cb84aae104c. Real input
and screenshots remain private. The temporary helper's partial images are not a product
pass or a reliable diagnosis of geometry.

The installed b7744b8 host store writes configuration to UserDefaults and the token to
Keychain, but reconnecting through its connection editor always requires a nonempty
token. It also attempts Keychain reads during app initialization and has no foreground/
unlock retry. These are concrete gaps; the precise physical reopen cause still needs
the new real relaunch check. Current run: /tmp/carlos-ep022/token-relaunch-01.log. Keep
its launch reset only for initial setup, then remove launch arguments before relaunch
so the test exercises the actual saved host/token. Require no extra native session or
prompt. The owner's installed version remains the early b7744b8 bundle.

The owner confirms that the early installed app and their resumed Claude CLI session
work together, then asks Root to match the HTML demo using that actual conversation.
The read-only reference at target/ios-artifacts/owner-claude-reference/snapshot.json
contains76 rows, including empty completed reasoning and adjacent distinct tool calls
and results. Never commit the conversation, token or screenshots. The live engine now
owns the owner's work: do not stop or restart it. Its actual shared Unix socket is
tmp/phone-preview-b7744b8/daemon.sock, exposed to the strict private daemon path through
an owned bind mount. The separate shared-socket-identity.json receipt records that change.

Source UI runs06–09 remain failed, not passing. Run06 spooled all10483884 original bytes;
its one-second main-thread sample was mostly idle, with350.8MB footprint and512.2MB peak
in a debug build. Do not call this proof of an endless CPU loop. Runs07–09 reached the
tail-existence assertion but timed out around Copy. A separate Linux-only real-daemon
check, /tmp/carlos-ep022/check-source-state.py, proves the streamed answer has done=true
and source.bytes=10483884. The missing item/completed hypothesis is disproved; keep the
valid bounded-delta fixture. Match Copy by its actual announced original byte count and
record read/pasteboard phases without source text before another diagnostic run.

Runs10–11 failed the tail-visible check, rather than proving a clipboard failure. Their
retained samples are mostly idle. Run12 puts the complete source's last paragraph and
Copy on screen after long Markdown blocks become siblings in the transcript's lazy
list. Its remaining failure is an exact automation-value mismatch: SwiftUI localized
10483884 to10,483,884. Use an explicit verbatim accessible byte count. Run13 reaches
Copy, sees Copied and writes the OS pasteboard in41ms, then hangs because the runner
reads UIPasteboard on its own main thread and cannot answer its cross-app Allow Paste
dialog. Both the screenshot and runner sample confirm this, not a product inference.
Keep source13-diagnostic and source-ui-13.log. Info-level OSLog records require --info.

The simctl pbcopy/pbpaste fallback segfaults on this Mac, even for a short sentinel in
run14, so it is removed rather than weakening the clipboard check or changing privacy
settings. Run15 keeps the original byte count/SHA256, real Copy/Copied/reset UI assertions
and original deadlines. It uses the SDK's explicitly Sendable UIPasteboard on a detached
task, while the runner answers the actual system Allow Paste dialog. Its result is
pending at /tmp/carlos-ep022/source-ui-15.log. Scripts keep the owned simulator booted
before XCTest and serialize all compiler/simulator jobs. No other simulator is touched.

Run15 failed at the tail precondition after viewport changes, before Copy. Its later
automation failures are downstream noise, not another clipboard defect or evidence of
an exact-copy pass. The async workflow now returns immediately after a failed required
tail/Copy check. Keep source15-first-hierarchy.txt: it shows an enormous measured
paragraph where the estimated bottom position should have been. Run16 passes in44.7s
after native scroll requests target the stable bottom id, and each newly decoded visible
block reports its real size change. The asynchronous reader/system-Paste handling and
all original clipboard assertions pass too. Do not re-label previous runs as passing.

EP021b ends at runtime `6de40a2`, checked documentation tree `fb5ed10`, source
`67a1a38a348a6343400594a6d06dbe4a`. Its exact checked CLI/daemon/helper/probe live under
`/tmp/carlos-ep021b/reviewed/final-pair/identity.json`; final evidence is
`.agents/reports/exec021b-performance-reviewed.json`. All225 daemon,330 terminal,23 native
Codex and10 packaged native Claude cases pass. Cold replay50.46ms, scroll p950.69ms and
typing p950.48ms remain within the original EP016 rules. The final authored production
count is97373. Do not compare with an arbitrary mutable target executable.

The Mac has Xcode26.0.1, macOS26.0.1 and an Intel CPU. Its default Rust was1.93.0 at the
read-only check; Linux/CI use1.100.0. Install/use the pinned additional toolchain under
the Carlos build directory when necessary, leaving the Mac's global default and other
projects untouched. Its owned simulator is named `carlos iPhone17 Pro`; discover its
current UUID with the script rather than copying another project's simulator address.

`carlos-client` currently bounds facts at64MiB and body data at8MiB across32 windows.
Choose the phone's facade/cache/error policy from its actual caller and measured memory.
Do not guess that these bounds permit another full Swift copy of every body. EP021b's
owner retains loaded older rows within existing caps. Claude can still scan the native
file before its first archive window; Grok still replays its native conversation in full.
These are carried limits, not permission to hide data.

The Mac’s system name lookup cannot resolve static.rust-lang.org or index.crates.io.
Direct dig queries get valid answers from its configured DNS server, while Python
getaddrinfo and curl fail; curl with an explicit address succeeds over HTTPS. This
narrows the failure to the system resolver but does not establish its underlying
cause. No resolver setting or service was changed. Linux can fetch the
official release. `scripts/ios-toolchain.py` verifies each Apple Rust component against
the release manifest and relays only those archives; the installer writes below the
owned build directory. `scripts/ios-prepare.py` stages only phone/client/protocol source
and Cargo-vendored locked dependencies. Its reduced workspace may remove optional
features unified by unrelated CLI crates, but it rejects any new package/version.
Published UniFFI0.32.2 includes macros without the main branch's macro-scaffolding
feature; the first build exposed that difference and the manifest now matches the
published crate. No runtime async feature is enabled.

The first Mac build compiled both Apple static libraries and generated the XCFramework,
then failed because XcodeGen calculated a framework path relative to the spec while the
project lived elsewhere. The spec is now copied into its build directory before project
generation. The generated Swift imports carlos_phoneFFI, so the C module map must use
that exact name. A missing libLLVM for rust-objcopy also produced stripping warnings;
the project-local official llvm-tools component fixes that toolchain dependency.

Putting vendor sources inside each versioned source directory caused Cargo to compile
all third-party crates again after an app edit. Stage them separately by their content
hash and give Cargo their stable absolute Mac path. Keep app sources versioned and
build outputs separate; no rsync deletion or broad repository sync is needed.

A generic simulator build also requests arm64, even on the Intel Mac. The package now
builds aarch64-apple-ios-sim and combines it with x86_64-apple-ios using lipo; the
XCFramework contains that universal simulator library and the arm64 phone library.
The specific owned-simulator test already passed before this packaging addition. Keep
the failed generic-build log instead of calling it a test failure. Never edit a running
Bash wrapper in place: one run reached its completed framework then stopped when its
shell read changed file offsets. The subsequent unchanged-source direct XCTest passed.

The first stable-vendor rebuild reused third-party packages: existing Apple library
builds fell to11.09s/7.82s, followed by the new arm64-simulator target and host generator.
Later existing-target rebuilds were7.70s/6.86s/6.47s. These are compiler-cache observations,
not app performance claims. Native compilation also caught Swift's Protocol metatype
name clash in the error constructor; use the specific InvalidMessage error instead.

The owner asked for an early usable iPhone build. Root estimated2–3hours for basic
sessions/read/send/approval/Stop, separate from complete EP022. Signing settings exist,
but the read-only physical-device detail query timed out after15seconds. Simulator work
continues; no phone unlock, installation or App Store publishing has been requested.

The owner subsequently unlocked the iPhone18 Pro, requested repeated access checks and
pointed Root to Stormvault's `.agents/knowledge/mobile-deployment.md`. That page records
the installed pymobiledevice3 native-tunnel/Cryptex1 image workaround for this exact
phone. Direct Apple queries timed out, and the documented mount failed before reaching
the image at RemotePairing.CreateAssertionCommand. Scoped resets touched only current-
account Mac-host device/pairing helpers, not simulator helpers; the login Keychain was
prepared through the documented stdin-only password path. No app or phone data changed.
The user then rebooted the Mac. A fresh hardware-UDID details query now confirms iOS27.2,
Developer Mode enabled, developer services available and the live localNetwork tunnel
connected. The system DNS lookup also succeeds again. The former USB/network question
is superseded. Proceed with the early phone build/install checks; keep EP022 open and
retain the simulator/full-plan gates. Never publish a store build under this test grant.

The first UI workflow exposed two test-selector errors: the switch's actual nested
control must be tapped, and the composer must be selected as a TextField rather than
an ambiguous Any query. The next run passed its visible answer assertion but its Python
post-check read the wrong trace family (rpc.jsonl instead of the actual NativeCodex
peer). Its retained codex.jsonl proves the exact Unicode prompt. The corrected complete
rerun passes both sides. Preserve those failed harness runs and their XCResult records.

The first held-reply UI run reached the native prompt but never found its approval:
the app did not follow appended content, so the lazy list left the card below the
visible area until the held native request timed out. The retained failure is
phone-905b8e12f997, source0ea129fe0e2f331fffac, ui-tests-20261116T180216.xcresult.
The corrected native scroll geometry and user-scroll phase checks preserve following
near the bottom, stop following beyond40pt and expose Latest beyond260pt. The actual
held-control rerun passes. Pixel-anchor/prepend/expansion tests still remain.

The first ten-megabyte UI fixture incorrectly emitted its entire answer as one native
Codex frame. The existing native8MiB guard correctly closed that connection; no source
reader or formatter success was claimed. Retain phone-7f2b296870c8 and Mac sourcea782741a4e88462421da,
ui-tests-20261116T183620.xcresult. The corrected fixture streams16K-character deltas and
leaves the native guard unchanged. The full terminal gate also first stops at a private
state-directory ancestor owned by UID1000 while this execution environment reports UID0.
Keep source-terminal-gate.log and run-dQnf4r. The rerun script is an exact copy of
scripts/test.sh with only cd and the artifact root changed to /tmp/carlos-ep022; this is
how the prior EP's preserved gate handled the same environment. No assertion, deadline,
permission check or runtime invariant was weakened.

## Decision Log

- 2026-11-17: request one complete Codex turn per native history reply. A turn
  can already contain tens of MiB, so a fixed100-turn batch cannot bound bytes.
  Keep whole-turn deduplication, the existing8MiB event cap,64MiB history cap,
  per-original-read budget and20s deadline. A single turn above64MiB still needs
  item paging in future work; do not claim this change removes that safety limit.
  Native contract tests continue checking Codex's100-turn API independently of
  Carlos's smaller read policy. The phone scrolling fixture groups its60 fixed
  messages into one turn; its history retry fixture groups the same130 sparse
  messages into two turns, preserving the same visible bodies and held-read boundary.

- Decision: preserve a saved server's ID when editing its address, but use a fresh
  transport/model for a changed endpoint until authentication succeeds.
  Rationale: matching only by address created a second saved entry. Reusing an old
  client with another server risks stale session facts/credentials. A failed edit
  must leave the working entry intact; a new endpoint requires an explicit token.
  Date/Author:2026-11-17, Root.


- Decision: use UIKit's plain compositional list and remove flow-layout gesture
  offset compensation. Retain only explicit idle history/tool anchors.
  Rationale: direct offset calls stop momentum; bounds-only correction retains a
  small backward step, and layout-context correction produces larger snaps. The
  native list passes moving-frame checks and normal/strong flick comparisons
  without a custom deceleration rate or another layout cache.
  Date/Author:2026-11-17, Root.


Decision2026-11-17 Root: use one native collection scroller, retain bounded parsed
inline documents and block facts, and defer structural updates until a gesture
ends. Remove the former SwiftUI marker controller instead of retaining competing
scroll paths. Keep syntax work off the UI thread and preserve exact source/copy
and2pt reader-position checks. The owner reports both stutters and jumps, so verify
both less per-cell work and position before calling the fix complete.

Decision2026-11-17 Root: replace per-row SwiftUI geometry observations with weak native
view markers. Reading those rectangles does not add a dependency back into the lazy
stack's placement graph. Use one-shot row jumps plus the existing measured-point
correction, not a bound programmatic ScrollPosition. Removing the binding alone did
not cure the stall; removing per-row geometry callbacks does pass the real scroll
workflow. This supersedes the earlier String-id/top-anchor experiment. Require the
same actual history and full workflow checks before accepting the replacement.

Decision2026-11-17 Root: honor the owner's new request to install the current progress
preview before closing EP022. Build/sign source8bcbe1b071c29b677fe6 separately, preserving
the old232 preview and owner daemon/CLI. Its live-scroll workflow passes; its35.67pt
failed-history-read jump remains disclosed and under repair. This install is not final
plan approval and does not replace the remaining simulator/performance/end-review gates.

Decision2026-11-17 Root: keep long tool output inside a320pt native scroll viewport,
with short results using their actual measured height. Split display text off-main
into64-line blocks and draw only visible blocks; do not change the checked output
window or original source. This adapter follows actual CoreText samples rather than
adding a second transcript engine or silently omitting displayed lines. Use explicit
top-left content alignment for a short result, proven by an exact file-text X check.

Decision2026-11-17 Root: use the current catalog for lifecycle availability when the
cached transcript cannot refresh after a stopped owner. Keep the old transcript for
reading; expose the real Reopen control instead of hiding it behind stale ready facts.


- Decision: try syntect5.3 with only bundled syntax grammars and the pure Rust
  fancy-regex engine in carlos-phone, not carlos-client/core. Before adding it, the
  isolated stripped Linux release probe measures2366352 added executable bytes,
  grammar load0.827/0.862/0.839ms and35,000-byte Rust parsing59.339/61.543/60.051ms;
  child peak RSS11208KiB (whole probe, not app or isolated library allocation).
  The probe includes default themes; the phone needs no themes, YAML, HTML, C regex,
  file reads or network. Classify only bounded visible snippets into semantic roles
  off the UI thread; Swift chooses Mocha colors. Unknown/oversized/error input keeps
  complete plain text. Require actual Mac build, dependency/size and UI evidence
  before accepting this pending syntax change. Probe files are under
  /tmp/carlos-ep022/syntax-cost; Mac syntax-native-01 fails on an incomplete vendor
  transfer, not a grammar result. The root-only sync exclusions preserve crate build
  sources. Date/Author:2026-11-17,Root.

- Decision: initialize the native scroll position with String row identities before
  adding paged history. Apple's ScrollPosition documentation says this lets SwiftUI
  update the topmost visible view identity and keep its position stable as content
  changes. The earlier edge-only position could not track those identities. Keep the
  successful tool/reader checks unchanged and rerun them after the history check.
  Date/Author:2026-11-17,Root.

Check actual hardware Return on a separate Carlos-owned iPad simulator, retaining
the existing iPhone simulator for all phone layout and flow checks. Apple’s current
XCUIElement overview documents keyboard/mouse tests for macOS/iPadOS15+, while
its iOS overview lists gestures. On the iPhone simulator, typeKey queries keyCommands
but delivers neither a UIPress nor an action; direct UIKit command tests pass. Do
not keep changing the editor based on an unsupported test path. Use the same app,
real authenticated daemon and exact native prompt assertions on the owned iPad.
The private env file /tmp/carlos-ep022/keyboard-mac.env sources the existing Mac
settings and changes only the simulator name/type. The new device is named
carlos keyboard iPad, UUID64004266-39DD-46F6-A8FF-6CFC64DCCE58, iPad Pro11-inch(M4),
existing iOS26.0 runtime. Only the Fixture app and test-runner targets advertise
iPad support; the normal Debug/Release app remains iPhone-only. Create/use it
under the shared Mac lock; never stop or alter another project’s simulator.
Reference: https://developer.apple.com/documentation/xcuiautomation/xcuielement .
Date/author:2026-11-17,Root.


Keep speech recognition native and private. The microphone requests permission only
on an explicit tap, checks support for the device language and requires on-device
recognition. It never chooses a remote recognizer or automatically sends words.
Stop/end/error preserve recognized text in the current draft; switching sessions
invalidates old callbacks. Retain the native recognizer for the recording lifetime.
Native permission dialogs may make the app inactive, so only backgrounding cancels
an authorization request. Stop active audio on inactivity/backgrounding. The owned
simulator check intentionally denies access; actual acoustic accuracy is a separate
hardware limitation and must not be claimed from this test. Date/author:2026-11-17,Root.

Keep private prompt placement in Swift as a small device-local layout hint. Capture
the last native row at Send, place an idle send’s private echo after that row, and
leave queued sends at the tail. Remove the hint when the real operation gains its
native row. Never invent, reorder or persist authoritative native history to make
a screenshot look right. Date/author:2026-11-17,Root.


Correction (2026-11-17,Root): the earlier Safari-heading diagnosis was wrong. Root
misread the reported source line, which points at the Allow-button query/await after
returning to Carlos. The original Safari text query already passes. UI02,03 and04 fail
because an empty subscription replay makes shared cached facts current without emitting
any screen update; PhoneHostModel retains currentSnapshot=false. The regression
an_empty_reconnect_replay_notifies_the_phone_when_cached_controls_become_current fails
before the shared-client fix (empty-replay-red.log). Client::confirm_subscription now
emits one metadata update after an accepted healthy subscription when the same batch
contains no session changes. It never grants freshness before the reply, polls, replaces
native state or resends a mutation. All200 client and24 phone tests pass afterward.
Browser05 restores both Safari foreground/actual-heading checks and all original consent
checks, plus an explicit assertion/capture after returning. Treat the earlier heading
queries as unnecessary diagnostic variants, not a browser failure or a weakened check.


Decision (2026-11-17,Root): keep simple offered approval buttons inline, but open
structured questions/forms in a native sheet. Long interactive forms must not depend
on a transcript lazy row's estimated frame, and Submit/error controls must remain
above the keyboard. Retain an explicit draft object in the live card when closing the
sheet. Close does not send a denial, approval or cancellation. Keep complete request
details available without filling the main conversation with native thread/turn ids.

Decision (2026-11-17,Root): move the existing flat MCP field description into the pure
protocol crate and have both the terminal and phone use it. Keep the daemon's complete
schema validation and backend integer checks authoritative. Do not copy a schema parser
into Swift, add another form crate or alter the wire format. The native form editor
captures the original complete schema, maps choice positions to exact original values,
keeps integer text in Rust and gives optional fields an explicit Include switch so
omission differs from an empty string/array or false. Disable a card locally while its
original submission is pending, without blocking Stop or pretending it resolved.

Decision (2026-11-17,Root): bind neutral question fields without asking Swift to encode
arbitrary native JSON. Capture the original field definitions, keep option values in
Rust and recheck the original live card before submission. Keep a user's edited field
state when the socket reconnects; a changed request disables the old editor and retains
its answers rather than interpreting old option positions against new values. Browser
completion is information only; opening a link never grants consent. Fields/browser
work precedes the remaining flat-form editor; do not broaden the wire protocol merely
to expose a screen that can use its existing typed input.

Decision (2026-11-17,Root): give expanded running and completed tools one native output
model using the shared checked read/watch/close calls. Keep original source separate.
Close offscreen output lifetimes and release their Swift text; retain only a measured
height until a visible row refetches. Keep each shared bounded suffix's omission notice
visible. Format classified colors off the main thread without writing a second terminal
control parser. Keep expansion/older-history pixel-anchor tests open until proved.

Decision (2026-11-17,Root): keep connection errors in the editable form and show an
explicit connecting state until the shared client accepts the daemon's greeting. Keep
Connect and errors above the keyboard. Bound the greeting wait to15 seconds, cancelling
only that attempt on timeout. Keep every saved token and private operation intact.

Decision (2026-11-17,Root): read saved tokens only while the app is active and protected
device data is available. Retry disconnected hosts on activation/unlock, and reconnect
after returning from the background to fetch current facts. Keep the same native client,
private-operation records and saved credentials. A blank token in an existing-address
editor reuses Keychain; an edited address requires an explicitly entered token. Save
host-name/private-network edits without replacing the host model. This fixes the observed
mandatory re-entry and first-host trap without adding another authentication framework.

Decision (2026-11-17,Root): use the owner's checked snapshot only as a private read-only
visual example. Keep real credentials off the simulator and do not send a prompt or
control to the running session. Fix the demo's concrete spacing, full-width user band,
centered header and width-measured compact reasoning. Hide only completed reasoning
whose complete inline text is empty; a source descriptor must still load its real text.
Keep distinct call/result rows distinct until an actual checked relationship exists.

Decision (2026-11-17,Root): flatten long answer/commentary documents into individually
addressed Markdown blocks in the transcript's single lazy list. Nesting lazy lists made
the huge answer's body blank while its footer was visible. Keep one original file and
one parsed Rust document, and attach checked reader lifetimes to visible blocks. Native
edge-based ScrollPosition and size-change anchors follow new content; body-only or empty
close batches must not trigger another host-facts read or scroll request. The current
viewport fixes the list width, and the user band's background bleeds to the screen edge
without widening its parent. The latest geometry/source checks remain pending.

Decision (2026-11-16,Root): keep one EP with small buildable milestones. Start with an
actual generated Swift caller and native app shell, then add only the interface needed by
each real screen. Do not generate a mirror of every public Rust method. Session listing
and lifecycle, greeting/reconnect, transcript/history/source/output, prompts/interrupt,
approvals, context display and Ralph are the initial screen boundary. Queue, host context
defaults, goals and native task panels remain Rust-only until a phone screen consumes them.

Decision (2026-11-16,Root): use bundled Inter and JetBrains Mono, dark Catppuccin Mocha,
portrait and iOS18 or later, matching the specified controls and sizes. Keep prose at the
specified15pt; an extra font-size setting is unnecessary. Store global tool mode/layout
on the device and per-row toggles by stable host/session/entry identity. Preserve toggles
across new entries and session switches. Follow the spec's explicit draft clearing on a
deliberate session switch; a disconnect must retain private draft/submitted text.

Decision (2026-11-16,Root): the connection screen accepts an engine URL, device token and
readable host label. Store credentials in the app's Keychain service, never logs or source.
New-session UI chooses a connected host and explicit cwd, with the current session's cwd
as a convenient editable default. Do not invent a remote file browser or hub. Use native
URLSessionWebSocketTask and its frame-size bound, with ordered writes and connection
generations. Preserve system TLS checks. Any private-network plain-WebSocket setting
must be explicit and documented for this daemon's WireGuard use.

Decision (2026-11-16,Root): use on-device iOS speech recognition for the mic. Request
microphone/speech permission on use, require on-device recognition and show an honest
unavailable state when the device/language cannot supply it. Never silently send audio
to a cloud service or add a server dictation protocol. A test-only transcriber can drive
draft/permission/error workflows; do not ship canned transcription as product behavior.

Decision (2026-11-16,Root): use a phone-only offline workspace and verified Apple Rust
components because the Mac's system resolver currently fails. Keep third-party source
paths stable across app edits so Carlos can reuse its own compiler cache. Do not change
the Mac's DNS service, global toolchain, other projects or simulators to finish this app.

Decision (2026-11-16,Root): the phone body handle contains the existing checked BodyToken
and a weak pointer to its exact client state. No additional token map, cursor parser or
body counter is needed. Every client call rejects a handle from another host, even if
both hosts have identical logical addresses and local counters. Changed-body batches
carry the actual current handles. Source reads cross the native boundary at most64KiB
at a time, while output reads retain the shared bounded/classified window. StaleRead is
a distinct ABI error, and explicit close returns all unsubscribe frames. Three new
Rust tests prove original Unicode/control bytes, foreign-host rejection and old-close
safety; all12 phone tests and phone clippy pass. The generated Swift app links this
larger real boundary in the passing held-control run. Native body consumers remain to do.

Decision (2026-11-16,Root): spool accepted original bytes to private device temporary
files, under a shared32MiB original-byte allowance for all currently open phone bodies.
The weakly owned opaque Rust handles still decide validity, revision and cursors. Swift
only appends their accepted UTF-8 slices to the private display/copy cache. A closed row
releases its file, and capacity/I/O failures preserve the accepted prefix. Preserve that
prefix while reconnect re-establishes a cursor; only an actual checked different source
revision can replace it. One formatter per visible document runs separately from the
socket queue, keeps only the newest pending target and parses a complete observed source.
This is bounded work, not a timed delta merger. Rust retains the parsed blocks and Swift
requests visible blocks only. Exact Copy reads the original file, not rendered spans.
Measure these policies on the actual app before final approval.

Decision (2026-11-16,Root): put the first actual Markdown parser in carlos-client, so
later native/web clients can reuse its semantic blocks. The phone only binds the block
reader. Use the already locked pulldown-cmark0.13.6 with default features disabled; no
package versions change. The graph guard reviews its pure parser dependencies, including
optional HTML/CLI formatting packages unified by the terminal in an all-features graph.
Keep the stricter protocol-only package set intact. Parse late reference definitions
against the whole source; keep code bytes, nested emphasis, table cells and list numbers.
Literal user/reasoning text uses the same block reader without Markdown interpretation.

## Outcomes & Retrospective

2026-11-17 resume repair in progress: Root traced the failure to one oversized
native history batch and implemented bounded complete-turn reads plus shared
cached-prefix reuse. Debug/release Rust checks and focused daemon/real-native
checks pass; full terminal/backend checks and a matching normal install still
remain. No phone app has been replaced and no owner daemon has been stopped.

At the owner's sandbox-restart checkpoint, the installed CLI/daemon/helper match
checked92d3a535 and the phone remains on owner-acceptedde74. The new server-manager
UI is buildable with24 native units green, but its complete removal UI check has
not yet passed because the original test matched nested native buttons. The
selector fix is saved for retry after restart. No normal/preview daemon was stopped.
The owner reports the new normal daemon idle. Root waits for the real writable
state/config/runtime mounts before continuing its phone listener/token handover.


The owner tests installed de74 on iPhone18 Pro and calls the scrolling "awesome".
This closes the physical jerk/momentum report. Keep the proven normal-session
behavior; the high10MB stress-case RSS and remaining whole-EP checks stay separate
open work. No physical recording was needed or started.


The native-list checkpoint fixes the measured momentum regression without adding
manual scroll physics. Clean source de74 passes19 native tests,1795 Rust tests,
the original2pt history/tool checks, exact10MB source/copy and10000-line code.
Both captured and generated histories match plain native fling travel within3%.
The checked phone build is signed and installed asde74; iOS denies auto-launch
while the phone is locked. Keep native-list-phone-{build,sign,install}-01.log.
The full final sweep, measured large-source RSS explanation, cost report and end
reviews remain open. No EP approval follows from this checkpoint.


The credential/caret checkpoint is checked by real UI, native13 and full Rust tests.
A pending connection holds an entered secret only until its own greeting succeeds;
failed edits leave stored credentials and host settings intact. Cancel resumes the
previous saved connection, old callbacks cannot commit a newer attempt, and unchanged
tokens are not rewritten. The app restores its existing native session across restart.
The native caret blinks as painted pixels and never becomes copied/accessibility text;
observed Stop removes it and keeps the partial answer and Copy. Completed text remains
selectable. Current-status labels, local-echo ordering, full scrolling, syntax/file
output, hardware keys, on-device speech and final release/performance/review gates
remain incomplete. Do not treat these checkpoints as EP approval.


Browser05 now proves the complete real Safari round trip and exact native callback.
The shared-client fix restores controls after an unchanged reconnect without adding a
Swift polling loop or bypassing current-snapshot checks. Root inspected the actual
Claude/Grok answer screenshots and browser return screen. Backend commit32e1e2e keeps
those native prompt checks repeatable. The final scanner should run only while an
actual turn exists; the browser image exposes a minor remaining animation polish item.
Remaining work includes syntax/file rows, streaming caret, hardware keys, speech,
full history/scroll checks, final release/performance checks and the end-only review.


The repeat full terminal gate02 passes completely with all225 daemon,330 terminal and
23 isolated native Codex checks. Preserve the initial Claude helper timeout as failed;
the isolated and full repeat use unchanged deadlines and pass. This gate covers the
shared form-reader move; later phone-only changes do not alter that terminal binary.
Native approval tests13/13 pass without skips onc2d300c443c311c7a02e,
tests-20261116T234228.xcresult. The new draft test uses actual generated Rust calls:
changed schema leaves private text and original fields intact, the old editor admits
no operation, and only explicit restart clears answers and captures the new request.
The native sheet asks before clearing; no request change silently transfers old inputs.
Browser02–04 show the real Safari page and unchanged consent card, but Allow stays
disabled after return. Root initially misread that as a heading-query failure; the
corrected source-line/activity evidence and shared-client fix are in the Decision Log.
Browser03 also folds raw browser details behind disclosure.
Browser05 now passes that exact native answer assertion after fixing empty-replay notifications.


The flat-form checkpoint passes the actual invalid-answer/correction flow and typed
native callback checks; questions also preserve edits across close/reopen. Native12/12
passes onbf5931634d358dedfdc9. Form UI08 and keyboard credentials01 prove top-bar Done
stays clear of bottom actions; credentials01 passes on6a989ed5956b51b33809,
ui-tests-20261116T233338.xcresult, local phone-f36969e569d7, with no runtime-warning
node. The earlier form run retained only its old connection-toolbar warning. Root
inspected all retained form states and the final keyboard layout. Integer overflow
now rejects locally instead of rounding and all23 phone tests pass. The full terminal
reader-change gate02 passes after all225 daemon,330 terminal and23 native checks. Browser consent,
remaining screen features, final release/performance evidence and end-only review remain
unfinished; this is a buildable checkpoint, not EP approval.


The native questions checkpoint is committed asd6045ba after12 native/18 Rust tests and
the actual question-answer workflow. The complete debug terminal gate plus isolated
Codex contract passes at questions-terminal-gate-01.log, artifacts source-terminal-F8UPH8.
That gate precedes the new shared form-field helper; repeat the relevant full gate for
the final form implementation. The existing core/protocol tests pass after moving the
field reader;23 phone tests pass with typed forms, exact large integers, false, omitted
versus explicitly empty fields, invalid values and stale schemas. Forms native/UI checks
are now running, not yet passing evidence. The phone remains on the checked2327 preview.

Running-tool expansion is now committed as6cf1ced after actual UI/native/Rust checks.
The saved-token/render preview is installed; launch while the physical phone is locked
was denied and does not block simulator work. The next checkpoint is actual native
question input, then remaining form/approval polish, syntax, keyboard/speech, full scroll
checks and final measurements/review. No reviewer has been run during the EP.

The saved-token regression is now reproduced and the first fix passes an actual
terminate/relaunch/foreground UI workflow. The physical saved-host list explains why
the old app reopens the wrong endpoint. Final host/session restoration and updated
native tests remain in progress; the installed phone app is still the early preview.

The owner now explicitly requires polished appearance, complete flows and durable saved
credentials before EP approval. EP022 remains incomplete. The larger UI renderer passes
real source/copy, prompt, held-control and native checks; credential relaunch, remaining
features and final reviews are still required. No new physical preview was installed
during the visual work, and the live owner engine/socket/token remain unchanged.

The owner now confirms the physical preview works with their actual Claude session.
The installed bundle still has the basic renderer; native Markdown/settings/source and
the current visual fixes are not installed yet. EP022 remains open. The full corrected
debug terminal gate and8 native units pass, but source Copy, later visual checks and the
remaining end-to-end/release/performance gates are not complete.

Small buildable commits dec3de5 and766276f add native display settings/activity and the
bounded original-body/Markdown reader. The latter's full workspace Cargo pre-commit
checks pass; the actual typed phone tests pass12/12 separately. The corrected full
release terminal gate is running under /tmp/carlos-ep022/visual-terminal-release-01.log
with the already documented artifact-root-only environment fix. It has not passed yet.

Update (2026-11-17 01:35Z): that complete release gate now passes, including21 architecture
cases, workspace release tests/lints/build, both75-test compiled Claude helper suites,
225 daemon cases,330 terminal workflows and selection/source-failure/dictation/ANSI
checks. Artifacts are /tmp/carlos-ep022/source-terminal-CR6gTR. Source10e1e1bf71d226124cce
also passes10 native tests with0 skips, the real prompt workflow and held approval/Stop
workflow. Repeat these narrower app checks for the final5635510dc076454c0a87 block-follow
change; the large-source/clipboard case already passes. No review or final approval yet.

The actual app now passes an authenticated connect/create/send/answer workflow against
a real daemon and deterministic native Codex peer. All9 Rust boundary tests and4 native
unit tests pass. The exact checked source also builds and signs for the owner’s physical
iPhone18 Pro, where Apple confirms successful installation. Launch awaits an unlocked
phone; no physical-screen or physical-backend success is claimed yet. This is an early
preview with basic plain rows and simple approval/Stop controls, not complete EP022.
Complete body/output/Markdown, scrolling, preferences, native inputs, dictation, the
remaining socket/backend tests and final gates before review or delegated approval.

## Context and Orientation

Read `AGENTS.md`, `.agents/CODE_STYLE.md`, `PROGRAM_PLAN.md`, this plan,
`docs/engine-architecture.md`, `docs/ios-build.md` and `tmp/main-screen.md`. The last
file contains the authoritative screen tokens, entry layouts, interaction rules and
acceptance list. The prototype is a reference, not app code. The architecture and this
plan state the running-tool and native-choice overrides.

`crates/carlos-client` is a pure state machine, with no sockets or timers.
`Client::begin_with_token`, `receive`, `read`, `submit`, `disconnect` and `write_failed`
own checked connection/reply state. `Effects` is must-use and contains outgoing frames,
changed session/body/operation IDs, replies and failures. `Replica` owns checked rows;
`Bodies` owns bounded source/output windows and captured body tokens. Source bytes remain
separate from normalized display output. `carlos-proto` holds wire records. Swift must
not reproduce their decoder, merge rules or native request/cursor/permission identities.

Add a boundary crate such as `crates/carlos-phone`, with only client/protocol, UniFFI
and the pure text dependencies the actual screens use. Keep bindings out of backend
Core. Expose typed records and narrow methods; do not export the entire command enum,
all panel families or a JSON string masquerading as the whole UI model. A received batch
returns outgoing messages and changed IDs; Swift reads changed/visible facts instead of
copying a growing full snapshot on each token. Keep ABI errors explicit, including cache
capacity, stale reads and fatal decode failures. The facade must preserve accepted text
and private submissions through each error. Use a small facade lock if UniFFI requires a
shared object, without adding a runtime, native backend, thread or second client controller.

Put the app under `apps/ios/`, with Swift source, resources, unit/UI tests and a small
XcodeGen project. Generate Swift/header/modulemap files from the same pinned UniFFI
library and package Apple static libraries into an XCFramework. Keep generated code and
build products separate from authored source. Pin dependencies in Cargo.lock; bundle
font files with their original licenses and source/hash notes. No Mac build may need a
network account to fetch fonts.

`scripts/ios-mac.sh` reads ignored `etc/mac-build.env`. It already provides check, ssh,
sim-create, sim-udid and sync. Read it before extending it: its generic sync currently
does not exclude every Rust build directory. Never copy `target`, node_modules, private
etc files or unrelated artifacts to the Mac. Prefer a tracked-source manifest and a
versioned source directory under `~/carlos-ios-build`, with shared build caches there.
Never use rsync --delete. Every SSH call uses ControlMaster=no and closes stdin unless
deliberately carrying data. Never change another simulator or kill another project's process.

## Plan of Work

### Milestone 1: prove the generated Swift boundary on the owned simulator

Recheck the Mac through `scripts/ios-mac.sh check`. Inspect the additional pinned toolchain
and required targets; install only what Carlos needs under its own build directory. The
Intel simulator needs x86_64-apple-ios, a phone needs aarch64-apple-ios; include arm64
simulator only when useful for the package. Leave the global1.93.0 default unchanged.

Create the smallest useful facade around the existing client and a minimal real SwiftUI
app. Generate Swift bindings in library mode, compile/link them on the Mac and run an
XCTest that calls the actual Rust library. Prove at least a greeting frame, accepted
reply, typed invalid-frame error and stale connection callback. Then boot/install/launch
only the Carlos simulator and capture its app screen. This must happen before expanding
the binding into a large speculative interface.

Add `scripts/ios-build.sh` or equivalent named commands for check, sync/build-framework,
build-app, test and launch. The exact script API can follow what the first successful
build establishes, but document it in `docs/ios-build.md`. Build an XCFramework and
prove both simulator and arm64 library slices. Simulator Keychain use requires ad-hoc
signing: CODE_SIGNING_ALLOWED=YES and CODE_SIGN_IDENTITY=-. Physical-device signing,
provisioning, TestFlight and App Store publishing are separate from this plan's ordinary
simulator checks. Never ask the owner to unlock a phone for a simulator-capable check.

### Milestone 2: connect and perform real session work

Use the deterministic daemon/native peers in `tests/daemon/support.py` and `tests/ui/`.
Run an isolated authenticated WebSocket daemon on Linux and let the simulator reach it
through the private network or an owned loopback SSH forward. Use generated test tokens
and temporary native homes, no user conversation or inference. Preserve both wire traces
and native requests under `target/ios-artifacts/` or a private /tmp evidence directory.

Implement connection entry, Keychain storage, session list/watch, subscribe/switch and
create. Keep saved/read-only facts visibly distinct from a current connection. Sending
allocates one shared operation, shows its local echo, preserves exact submitted text and
never automatically resends a mutation after an uncertain write. Retired socket callbacks
cannot affect a replacement connection. Reconnect uses the existing library's private
reply rights, event sequence and refresh behavior. A UI-owned queue can serialize socket
writes, but it cannot become another protocol/retry controller.

Test successful send, two devices sharing one session, disconnect during write, daemon
replacement, explicit reopen of a closed session when needed, invalid token and exact
draft recovery. Session switching clears the composer/stops dictation as specified;
network failure does neither. Starting approval cards remain answerable before Ready,
while early sends remain blocked by honest session availability.

### Milestone 3: render and control the specified session screen

Implement both Flow and Gutter with the supplied colors, fonts, spacing, touch sizes and
safe areas. Render user, reasoning, commentary, Markdown answer, tool, approval and notice
facts. Choose one-line versus long reasoning for the actual phone width. Keep original
answer Markdown for Copy, with a1.4-second per-answer Copied state. Complete source may
require bounded reads; do not mistake a display window for the entire copy source.

Use shared tool kind/status/summary/diff facts. Literal labels truncate, AI labels wrap,
unknown outcomes remain unknown, and native fileChange/MCP data stays complete. Full
starts expanded, Auto/Compact collapsed; running tools may expand when source/output is
available. Fetch/read/watch only visible or explicitly requested bodies, respecting
original tokens, revisions, offsets and budgets. Horizontally scroll unwrapped output,
keep typed line colors and syntax colors, and retain accepted prefixes while new bytes
wait or a retry fails. Measure the size and cost of any syntax dependency before adding it.

Expose only actual offered approval choices and structured inputs. Reuse shared validation
where present rather than clone a terminal controller in Swift. Preserve original card
identity, first-answer-wins and private inputs. Show unsupported inputs honestly with
native cancellation when offered. Resolved rows must describe an observed decision;
retirement without a decision is not proof of denial. If an actual screen needs a fact
the client currently discards, add that narrow checked fact with meaningful tests.

Implement Send/Stop, Return versus Shift/Option+Return, the unsent session-name placeholder,
context/model/quota/timer and Ralph. Sending during a running turn uses the specified
queue policy and never silently becomes an edit. Ralph follows the owner's actual pending
and accepted state. Source loading must not replace the activity line or flash its labels.

### Milestone 4: preserve place and device-local behavior

Prove scrolling with an actual simulator early: bottom on open, follow within40pt, stop
following beyond40pt, Latest beyond260pt, and stable visible entry/pixel offset across
older prepends, expansion, streaming and keyboard changes. Use native lazy rendering and
the smallest platform adapter needed by observed behavior; do not write another transcript
engine. UI tests should assert actual anchor positions, not only that a reply was recorded.

Finish sessions/settings sheets, global tool/layout preferences, per-row toggles, safe
areas/keyboard and on-device dictation. Starting/stopping the mic appends recognized text
with the specified spacing while retaining the existing draft. Cancel on deliberate
session switch. Permission denial, missing on-device support, interruption and recognition
failure must leave private text safe and show a concrete state. Simulator tests can inject
audio/text results at the native speech boundary, clearly separated from product behavior.

### Milestone 5: complete checks, measure and review

Run actual Swift unit/UI tests against the real Rust library and actual daemon, including
all three deterministic backends, large/older/streaming source, offered/structured/retired
approvals, held reads, reconnect and private draft cases. Capture screenshots, request
traces, test results, exact build hashes and scripts. Add each broken phone workflow to
the repeatable simulator suite, without reducing its assertions to make it pass.

Run cargo test after meaningful Rust changes and the complete release/terminal/native
gate before closure. If a wire/client behavior changes, keep terminal checks for it.
Measure current TUI/headless behavior against the exact EP021b pair, also retaining the
original EP016 limits. Freeze checked bytes before timing; no builds/tests/hooks/reviewers
run alongside timing. Measure the real simulator's startup, typing, scroll, streaming,
body/fact memory, idle CPU, bridge cost and app/framework size. Report simulator figures
as simulator figures, not physical iPhone promises. Count authored Rust/Swift production
growth separately from tests, generated bindings, resources and build scripts.

Only when the whole app is ready, perform Root's engineering review and one Fable5.2
medium whole-roadmap review under the installed claude-subagents skill. Supply the whole
phone/web objective, actual final diff and all checked evidence. Resolve concrete bugs,
explain deferred optional choices and avoid reviewer cascades. After required checks and
Root's follow-up pass, record delegated approval, archive this plan unchanged and report
any remaining manual physical-device or landing context. Do not merge or publish the app.

## Concrete Steps

Run local commands in `/var/home/wegel/work/perso/carlos-daemon`, always through RTK:

    rtk git status --short --branch
    rtk proxy bash scripts/ios-mac.sh check
    rtk proxy bash scripts/ios-mac.sh sim-udid
    rtk cargo test --workspace --all-targets
    rtk cargo clippy --workspace --all-targets -- -D warnings
    rtk proxy bash scripts/test.sh --release --codex-contract --claude-contract
    rtk proxy python3 /tmp/carlos-ep019a/check-all-functions.py

Milestone1 adds the documented iOS build/test commands and their fixture harness. Use
those commands after they exist; do not claim a generated project or app binary is already
present. Run remote commands through the script, with project-local Rust homes and
explicit owned source/build paths. Keep a capturable log for long builds and emit useful
progress without waiting silently for more than a minute.

## Validation and Acceptance

A new checkout with the ignored Mac settings and documented prerequisites can generate
the bridge, build both required Apple library slices, compile the SwiftUI app and run
repeatable simulator tests. The source spec's complete acceptance checklist is satisfied,
subject only to the documented running-tool/native-choice/observed-stop overrides.

The simulator can connect to a deterministic daemon, create/select a session, display
each entry kind in both layouts, copy exact Markdown, expand a running/completed tool,
page all older supported rows, keep its visible anchor/private draft, answer actual
native choices, interrupt and toggle Ralph. The backend traces prove one intended send
and exact original controls, with no inference, hidden approval or mutation replay.
Keyboard, disconnect, cache/read failures and microphone errors remain usable and honest.

Rust/terminal/native checks and original performance limits pass. Actual simulator timing,
memory, idle and size evidence is recorded against exact artifacts. The final report
lists limitations, review scope/findings/decisions and reproducible commands. No compiled
phone dependency contains terminal, native backend, Whisper or Claude-helper code.

## Idempotence and Recovery

Build/sync into owned, versioned Mac paths and reuse only Carlos caches. Re-running build
or tests must not delete another project's data, rewrite native conversation files, stop
another simulator or change the global Rust default. Never rsync --delete. Keep real
tokens, SSH/signing keys and private host settings outside Git/logs.

Discard stale connection/body callbacks using their original captured identities. Preserve
private draft/submitted text on network, capacity and decode failures; do not treat a
reconnect or tap retry on a read as permission to resend work. An explicit new mutation
gets a new shared operation after the user chooses it. Preserve failed test traces and
fix the concrete cause before retrying. Do not relabel failed/partial gates as complete.

## Artifacts and Notes

Use `.agents/reports/exec022-*` for final reports, source/size counts, review verdicts and
compact measurements. Keep raw logs, screenshots, XCResult bundles, generated bindings,
frameworks and app products under ignored target paths or private /tmp paths. Record the
exact Linux source marker, Cargo.lock, remote toolchain/Xcode versions and binary hashes.

Primary references checked during preparation include the
[UniFFI proc-macro guide](https://mozilla.github.io/uniffi-rs/latest/proc_macro/index.html),
[Swift binding generator](https://mozilla.github.io/uniffi-rs/latest/swift/uniffi-bindgen-swift.html)
and [Apple scroll-position documentation](https://developer.apple.com/documentation/swiftui/view/scrollposition(_:anchor:)).
Use generated library-mode bindings and prove actual variable-height scrolling on the
simulator; a documentation promise alone is not the anchor test.

## Interfaces and Dependencies

The phone boundary depends on `carlos-client` and `carlos-proto`, with pinned UniFFI and
only pure text dependencies the specified screens consume. Existing Client/Effects,
Replica, Bodies, operation/reconnect and native-identity semantics remain authoritative.
Swift owns sockets, platform clocks/UI, secure device settings, clipboard and speech.
No new daemon/native owner, server audio service, hub, Web UI, Kotlin binding, speculative
panel screen or blanket protocol schema export belongs in this plan.

Revision2026-11-16: write EP022 in full after EP021b delegated approval and unchanged
archive. Start with the smallest real Swift consumer, then grow the specified app through
checked milestones on the owned Mac, keeping the shared-client and review boundaries.

Revision2026-11-16: record the small Rust/Swift boundary and Mac DNS failure. Add a
verified offline build path rather than changing the Mac's global toolchain or network.

Revision2026-11-16: record the real authenticated app workflow and explicit early-phone
test request. The owner's Mac reboot restored live iPhone18 Pro access and DNS; prepare
the phone preview without claiming EP022 complete or starting its end reviews.

Update (2026-11-16 22:55Z,Root): recorded the successful physical install and early
preview without closing EP022. Mac receipts are device-check/iphone18-install.json and
iphone18-launch.json below ~/carlos-ios-build. The signed executable SHA256 is
5cb981e4bb5c43f8327c533a6348b47dc89c187952a25643ad814e9c8c5e82bc. Its profile and
identity receipt are beside previews/b7744b8a156a9c2b8287-20261116T225036Z/Carlos.app.
The separate engine receipt is target/phone-preview-b7744b8/identity.json, with private
state and token under /var/tmp/carlos-phone-preview-b7744b8. Mac-to-listener reachability
and an authenticated empty catalogue read pass. A first state location under the home
directory was rejected because this execution environment maps the home ancestor to a
different UID; the daemon’s ownership checks were preserved, and the owned /var/tmp
directory passes them. The first hand-written health check used watch rather than
sessions/watch and correctly failed; the corrected check passed. No user session was
started. Keep this daemon/package alive for the owner’s test and do not replace it.

Update (2026-11-16 23:10Z,Root): recorded the actual held-control test, its initial
scroll failure and the checked body-reader boundary. Continue native long-body/output
rendering and complete the remaining phone screens; no intermediate reviewer was run.

Update (2026-11-16 23:43Z,Root): recorded checked native settings/source milestones,
current original-byte/formatter policies and the two retained gate failures. The installed
phone preview remains the original b7744b8 bundle. Its executable is3977344bytes and its
signed app bundle totals6067945bytes before adding a syntax-color library. Current source
UI rerun is source-ui-02.log, Mac a782741a4e88462421da/ui-tests-20261116T184122.xcresult.
Full terminal rerun is source-terminal-gate-02.log. Neither pending check is called passing.

Update (2026-11-16 23:55Z,Root): shared Markdown/FFI code is committed in51b6f10;
settings/native-source views remain a working milestone. The complete corrected debug
terminal gate passes: source-terminal-gate-02.log, artifacts source-terminal-Ngvd6e,
all225 daemon/330 terminal cases, selection2/source failures5/dictation2/ANSI1 and both
75-test compiled Claude helper runs. Native units8/8 passed separately. No release/native
contracts or final performance gate is claimed yet.

The second source UI run uses valid bounded native deltas and reaches idle, but its
transcript is blank; retain phone-04674c5b2765/a782741a4e88462421da/ui-tests-20261116T184122.xcresult.
A third variant uses native size-change anchoring and shows older history instead of
following the new source; a live screenshot is target/ios-artifacts/source-live-03.png.
Its app sampled257392KiB RSS/100% CPU and had no active source spool files at that instant,
so this is a scroll/view-lifetime failure, not proof that a10MB body needs a larger cache.
Keep phone-2ef17f67b3bc/92b1251fd7e7162b508f/ui-tests-20261116T184823.xcresult.
Root conservatively interrupted the next run while checking the third run's exit.
The retained timestamps show no concurrent simulator XCTest: run3 ended18:51:16 Mac
wall time; run4's result bundle began18:51:45. Correct the earlier in-band assumption
of an overlap. Run4 is interrupted, not passed, at d41e79f991863aec8aa7/ui-tests-20261116T185145.xcresult.

The current source-ui-05.log tests e65db7e92af8450409c6/ui-tests-20261116T185422.xcresult.
It uses native bound scroll ids and one deferred follow after actual data/formatter
changes rather than recursive geometry-size scroll calls. Parent accessibility ids no
longer overwrite Copy identifiers. The common ios-build.sh now holds an advisory lock
for the shared Mac/simulator, and the UI test uses a180second execution limit while
keeping every existing visible/native/clipboard assertion and its original wait intact.
Do not start another simulator run until this one has exited. If native lazy rows still
cannot hold the actual last source paragraph, inspect the retained screen/file state
and fix the view structure; do not increase waits, hide source or weaken Copy checks.

Revision2026-11-17: checkpoint the checked native composer, on-device speech error flow
and stable reader position. Keep hardware synthesis unresolved and begin older history;
no reviewer or EP approval has occurred.

Revision2026-11-17: add the phone top-of-history flow and exact failed-read identity,
then check real native rows instead of simulating history inside Swift.

Revision2026-11-17: record the actual Ralph pass and retained history jumps; keep the
new source check pending until its exact source/result are known.

Revision2026-11-17: strengthen the history check with a held real native page and
exact coordinates; add a project-local raw HID test tool after proving its API exists.

Revision2026-11-17: checkpoint the real history failure/retry/prepend pass, retain the
first-character failure and measure the pending grammar library before adding it.

Revision2026-11-17: record the actual Mac grammar test and inspected Ralph screens.
The first-character keyboard issue still reproduces after the binding guard; a small
fixture-only scalar/length trace checks where UIKit loses it. That guard is not called
a proven fix, and every temporary trace must be removed before the clean final run.

Revision2026-11-17: share the existing pure read-gutter parser and record the proven
first-character loss; check the smaller render-marker fix with the unchanged real UI.

Revision2026-11-17: record the native scroller passes, physical-phone feedback and
measured22.4% CPU reduction from shared parsed rows; keep motion quality and the final
source/functional sweep open. No reviewer or EP approval has occurred.

Revision2026-11-17: prevent the transient source cell from rendering a complete
answer before the parent splits it into native block rows. The unchanged10MB
source/exact clipboard check now passes; retain the prior main-thread stall.

Revision2026-11-17: record the complete candidate checks and signed physical-phone
install of9a45429810146c725110. Continue the final same-source app sweep and keep
physical motion quality open until it has direct evidence.

Revision2026-11-17: the owner confirms the installed source still jerks and asks
Root to see the defect before another fix. Pause broad checks, retain every pass,
and record moving frames from the real captured transcript. Settled positions
and CPU figures are explicitly insufficient for this bug.

Revision2026-11-17: add a temporary opt-in app-only phone recorder and deeper
read-only simulator gestures to observe the reported jerk. No additional scroll
fix is chosen until moving evidence reproduces it. Keep denied Mac capture and
the corrected callback warning honest, and remove temporary diagnosis code later.

Revision2026-11-17: confirm an actual16pt backward snap from exact decoded
moving frames and add a narrow native-layout trace before any correction. The
phone recorder is installed but remains idle pending the owner's timing reply.

Revision2026-11-17: rule out reparsing and the unused custom correction callback
for this reproduction. Test one native layout-pass anchor with the same deep
recorded gestures; no phone install or smoothness claim is made for it yet.

Revision2026-11-17: save the actual before/after moving-frame comparison. The
inspected backward scrolling snaps disappear in the candidate; native/Rust/history
checks pass. Keep long-source checks, phone install and final EP gates pending.

Revision2026-11-17: retain the owner-confirmed a665 momentum regression. Add a
real fast-flick travel check and compare a bounds-only correction with the slow
setter before any further deployment. Keep UIKit's normal deceleration settings.

Revision2026-11-17: retain the measured881pt/1163pt flick comparison and the
failed absolute native-control threshold. The owner requires normal iOS behavior,
so compare identical coordinate gestures directly with a fixture-only native
text panel, within10%, rather than imposing an arbitrary faster-than-native floor.

Revision2026-11-17: record the passing native flick comparison and the remaining exact-frame3pt reversal; test native layout-context offset changes before another phone install.

Revision2026-11-17: retain the799 native-context motion failure and test native compositional list cells without custom gesture-offset correction.

Revision2026-11-17: record the native-list movie, preserve the raw-offset comparison failure, measure visible-cell travel for both normal and strong flicks, and remove the unused temporary phone recorder.

Revision2026-11-17: record native normal/strong flick parity within3%, preserve the slow baseline failure, and freeze recorder-free source de74 for clean gates and phone packaging.

Revision2026-11-17: record clean native/history/live-tool passes and the visible-text slow-setter failure; add deterministic generated-history checks without private captures.

Revision2026-11-17: retain the clean10MB/exact-copy pass and its high sampled RSS as an open final-cost question; continue the checked native scrolling phone preview independently.

Revision2026-11-17: record the deterministic public flick pass and matching slow-baseline failure, with repeat commands in docs/ios-build.md.

Revision2026-11-17: update the discoveries, chosen native-list design and checkpoint outcome after all five clean scrolling gates pass; start the checked phone build.

Revision2026-11-17: record the checked native-list phone install and locked-device launch result; begin one separate prefetch-only large-source memory experiment without changing the installed candidate.

Revision2026-11-17: record the owner’s successful physical-phone scroll check and close the moving jerk/momentum task while keeping the remaining EP checks open.

Revision2026-11-17: record the explicit normal-install request, read-only mount blocker, checked shared installer and outstanding daemon-route preference; retain the measured prefetch memory reduction without changing accepted phone scrolling.

Revision2026-11-17: save the owner-requested sandbox restart checkpoint, verified normal install, idle local-only daemon facts, server-manager changes and exact UI test-selector retry state. No EP approval or new phone deployment.

Revision2026-11-17: resume after the sandbox restart and prioritize the newly reported oversized Codex history response; diagnose with isolated private copies before modifying paging or limits.
