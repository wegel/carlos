# EP022: build the actual iPhone client on the shared Rust client

This living ExecPlan follows `.agents/PLANS.md`. Keep Progress, Surprises & Discoveries,
Decision Log and Outcomes & Retrospective current. Root works directly on `iphone-spike`
in `/var/home/wegel/work/perso/carlos-daemon`, without developer subagents. The owner
delegated approval after complete checks, Root's engineering review and one end-only
Fable5.2 medium whole-roadmap review. Do not request intermediate reviewers or routine
approval. Do not install the spike CLI, merge into main, alter an archived plan or replace
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

- [x] (2026-11-17) Credential staging and trace-free caret pass on589f3adae575cb5d84ca:
  stream04 (ui-tests-20261117T003806), rejected-new-token repair
  (ui-tests-20261117T004433), saved-token/session relaunch (004534) and rejected
  replacement/Cancel/cold-restart (004648) all pass their exact native traffic checks.
  Local artifacts are phone-codex-527840feb882, edd76833420f,522b48d0af0e and8562385c4e48.
  All13 native tests pass without skips at tests-20261117T003735.xcresult; full workspace
  cargo test passes at native-polish-cargo-01.log. No traces remain in product code.
- [x] Current-status polish compiles and passes all13 native checks on89ce36e8545f4ba7b2ed,
  tests-20261117T005608.xcresult, current-status-native-01.log:
  connecting/loading session labels and a muted status dot must distinguish retained
  facts from a fresh current snapshot. This does not change native state or controls.

- [x] Protect a working saved token when the person types a rejected replacement (token-update03 passes).
  Direct code inspection shows PhoneStore.connect writes Keychain before the greeting
  accepts that token. Add a real existing-host rejection/Cancel/relaunch workflow, then
  commit the new credential only after its exact connection authenticates. Keep this
  native attempt small; no Rust outbox, persistent operation ledger or auth wizard.

- [x] (2026-11-17) Reconnect commit4d60ba7 passes the fresh full Rust/terminal/Codex
  check at reconnect-terminal-gate-01.log, artifacts source-terminal-PYEmq3:
  all225 daemon,330 terminal and23 native contract checks pass unchanged. The empty
  replay regression and actual Safari round trip pass as recorded below.
- [ ] Finish the next visual checkpoint. Source4068648d0d3bd0edc21f compiles the opaque
  1024px icon, native named launch-background asset and scanner’s actual-turn guard;
  native test bundle tests-20261117T000834.xcresult passes with13 tests. The editable SVG and PNG
  need no build-time image dependency. A native TextRenderer is being checked to paint a green caret
  after the last streaming paragraph without adding a source character; only the
  live final block uses a clock, and Reduce Motion keeps it steady. Stream UI01 on
  5657c2c5ddec92e37f86 fails its blink-pixel check; UI03 checks selection after UI02 shows ticks without paint. It checks actual changing green pixels, unchanged
  accessibility text and original Stop/partial-answer/Copy behavior. A test-only main
  actor warning in its screenshot helper is corrected locally; verify the final run.
  Root inspected visual-polish-launch-01’s actual initial screen oned1692fe352dba5ba41a. Its low-contrast placeholders and technical setup copy are improved locally; launch02 checks those changes.

- [x] (2026-11-17) Browser05 passes on5621087dafe487fb5108,
  ui-tests-20261116T235815.xcresult, local phone-codex-7ac042eb4735. It opens Safari,
  reads the actual page heading, returns to usable unchanged approval controls, sends
  no consent on page visit and exactly one original native accept after Allow. Stop
  stays disabled without a turn. Root inspected the return screen; no runtime-warning
  node is reported. All200 client and24 phone tests pass with the red/green empty-replay
  regression. Native tests13/13 pass on the same source, tests-20261116T235956.xcresult.
  The fresh full reconnect-terminal-gate-01 check is still running.

- [x] (2026-11-17) Actual Claude and Grok prompt workflows pass on013def396f66309592be:
  ui-tests-20261116T235051 and235216.xcresult, local phone-claude-7a77458b4096 and
  phone-grok-e57273ab1b52. Each selects its actual backend, creates one native process,
  sends exactly phone exact prompt 🦀 once and shows the matching native answer. No user
  account/session or inference is involved. Inspect the retained screenshots before
  final UI approval. The test script separates native setup, traffic reads and assertions.

- [x] (2026-11-17) Commit27853ca adds flat native form fields, exact integer/boolean/enum
  replies, explicit optional omission, stable sheets with retained private edits, per-card
  pending states and top-bar keyboard actions. Full cargo test passes in its commit hook;
  actual form/question/credential and native evidence is recorded below.
- [x] Finish browser approval checks with an owned local Safari page (browser05). The app now reads
  whether an actual native turn exists so browser consent without a turn cannot offer
  enabled Stop. All23 phone tests pass after this narrow facade fact. Browser fixture01
  captured a wrong test selector variable before the correction; its frozen source must
  stay unchanged. Browser02–04 reach Safari but leave Allow disabled after return; browser05 checks the shared empty-replay fix. Opening the page
  must leave native consent unanswered, then exactly one explicit Allow response must
  reach the original native callback. No external website, account or inference is used.

- [ ] (2026-11-17, owner steering) Treat saved credentials and finished UX as hard
  acceptance criteria. The owner reports entering a token again when reopening the
  installed early app and explicitly expects a polished app at the end of EP022.
  First reproduce a real terminate/relaunch and saved-token reconnect in the simulator;
  inspect physical saved settings read-only when available. Never clear credentials on
  an ordinary disconnect or force the owner to type a saved token again. Check actual
  app screenshots and complete flows, not only whether controls exist. Do not install
  the already built intermediate5635510dc076454c0a87 phone product as the final fix.

- [x] (2026-11-16) Close EP021b under the owner's grant, archive its completed plan
  unchanged and retain its corrected gate06/fresh frozen-pair baseline.
- [x] (2026-11-16) Read the full phone spec, architecture, client API and Mac build notes.
  The earlier read-only Mac check passed. No app source or Mac build was created during
  EP021b; preparation notes are `/tmp/carlos-ep021b/ep022-preparation.md`.
- [x] (2026-11-16 21:10Z) Prove the small real Swift caller before expanding the UI.
  Both Rust boundary tests and both Swift XCTest cases pass for actual greeting/reconnect
  rights, typed invalid-input/frame errors and stale socket callbacks. The SwiftUI shell
  compiles and links both generated Swift and the actual Rust static library. Evidence:
  `/tmp/carlos-ep022/first-swift-test.log`; Mac bundle3303d44c00c224ba856c,
  tests-20261116T160903.xcresult, owned simulator4C821FE4-54D8-452D-A46C-3187AAED6C61.
- [x] (2026-11-16 21:35Z) Finish the repeatable offline build/cache path and capture the launched shell.
  Completed: project-local pinned Rust/LLVM tools, both Apple library slices,
  generated Swift/headers/XCFramework, XcodeGen project and successful real simulator
  test. The launched shell's actual screenshot is
  `target/ios-artifacts/first-shell/screen.png`. The current wrapper passes generic
  Intel/arm64 simulator app compilation and both actual native tests with all three
  Rust targets: bundle324d2d6bcadf1f77cbea, tests-20261116T163525.xcresult.
- [ ] Connect the actual app to a deterministic daemon over WebSocket; prove greeting,
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
- [x] (2026-11-16 22:55Z) Build, sign and install the checked early preview on the owner’s
  iPhone18 Pro after the Mac reboot restores live device access. The exact simulator-checked
  bundle b7744b8a156a9c2b8287 builds Release for arm64; the existing development profile
  includes this phone, and strict signature/entitlement checks pass. Apple’s install receipt
  confirms app.carlos.phone. Launch was denied only because the phone locked again; the
  owner has been told to unlock/open it. A separate frozen EP021b daemon is ready on the
  private LAN, with a new private device-token file and an empty catalogue. No model call
  was made. Complete app acceptance remains below.
- [ ] Implement the specified header, transcript, composer, activity line, Latest button,
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
- [ ] Complete stable scrolling, running-tool expansion, Markdown/copy, Ralph, native
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
- [ ] Run actual simulator workflows with daemon/native traces, memory/latency/size checks,
  Rust and terminal/native gates, then freeze exact checked artifacts.
- [ ] Complete Root's engineering review and the single end-only Fable review, resolve or
  explain every finding, record delegated approval and archive unchanged.

## Surprises & Discoveries

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
