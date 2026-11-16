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
- [ ] Complete stable scrolling, running-tool expansion, Markdown/copy, Ralph, native
  on-device dictation and honest permission/unavailable states.
- [ ] Run actual simulator workflows with daemon/native traces, memory/latency/size checks,
  Rust and terminal/native gates, then freeze exact checked artifacts.
- [ ] Complete Root's engineering review and the single end-only Fable review, resolve or
  explain every finding, record delegated approval and archive unchanged.

## Surprises & Discoveries

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

## Decision Log

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

## Outcomes & Retrospective

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
