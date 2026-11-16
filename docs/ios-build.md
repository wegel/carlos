# Building the iPhone app on the remote Mac

The iPhone app (ExecPlan 022 in `docs/engine-architecture.md`) compiles on a
remote Mac over SSH. The development machine runs Linux and cannot run Xcode.
The same Mac builds the owner's other iOS projects, so carlos keeps its own
directory and its own simulator there.

EP022 has started. `apps/ios` contains the native app and real-library Swift
tests; `crates/carlos-phone` binds only the shared client calls those screens
need. The first milestone proves the generated Swift caller before adding the
full phone UI. An app shell is not yet the completed session screen.

## Settings

`scripts/ios-mac.sh` reads `etc/mac-build.env`. Git ignores `etc/`, because the
repository is public and the file names a private host and key paths.

- `MAC_BUILD_HOST`: the Mac's `user@address`.
- `MAC_SSH_KEY`: the private SSH key path.
- `MAC_BUILD_DIR`: a relative directory below the Mac home, normally `carlos-ios-build`.
- `IOS_SIMULATOR_NAME`, `IOS_SIMULATOR_TYPE`: the simulator Carlos owns.
- `DEVELOPMENT_TEAM`: the Apple team id for physical-device signed builds.
- `MAC_KEYCHAIN_ENV`: the local mode0600 file defining `KEYCHAIN_PASSWORD`.
- `ASC_KEY_ID`, `ASC_KEY_FILE`: an App Store Connect API key for physical-device
  builds that need to refresh a profile.

A new checkout needs a new `etc/mac-build.env`. Ask the owner for the values.

## Commands

    scripts/ios-mac.sh check          # macOS, Xcode, simulator, free disk
    scripts/ios-mac.sh ssh <command>  # one command on the Mac
    scripts/ios-mac.sh sim-create     # create the carlos simulator if missing
    scripts/ios-mac.sh sim-udid
    scripts/ios-mac.sh sync <dir>     # copy an app source directory to the Mac
    scripts/ios-mac.sh fetch <path> <dir> # retrieve a Carlos screenshot or test bundle

Run these commands through `rtk proxy bash` from the spike checkout:

    scripts/ios-build.sh check
    scripts/ios-build.sh toolchain   # once: install project-local Rust1.100.0
    scripts/ios-build.sh framework   # phone and Intel/arm64 simulator slices, generated Swift
    scripts/ios-build.sh build       # framework and native app
    scripts/ios-build.sh test        # actual Rust-backed Swift tests on owned simulator
    scripts/ios-build.sh launch      # build, install, launch and capture screen.png

The scripts stage a small workspace under `target/ios-source/<hash>` containing
phone/client/protocol sources, the app, its build script, license and dependencies.
Cargo verifies the offline vendored crates. The script checks that their versions
match Cargo.lock, while removing features selected only by other workspace members.
No terminal, backend, helper, private `etc` settings or Linux build products are
copied. The Mac receives the bundle under `sources/<hash>` and builds under
`builds/<hash>`, with shared caches in `rust-target` and `toolchains/cargo`.
Verified third-party sources live separately under `ios-dependencies/<hash>`;
their stable paths let Cargo reuse them after an app edit.
Generated Swift, headers, XCFrameworks, Xcode projects, DerivedData, screenshots and
XCResult bundles stay under those ignored build paths. Local logs and the most
recent source hash are under `target/ios-artifacts`.

The Mac currently cannot resolve Rust or crates.io hosts. Linux downloads the
official Rust1.100.0 release manifest and verifies each required Apple component's
SHA256 before copying it. The installer writes only `toolchains/rust-1.100.0` and
leaves the global Rust default untouched. Vendored crates let all subsequent Mac
Cargo commands use `--offline --locked`. No DNS or network setting needs changing.

## State of the Mac (checked 2026-11-16)

- Intel Mac, macOS 26.0.1, Xcode 26.0.1, Swift 6.2, `xcodegen` in `/usr/local/bin`.
- iOS 26.0 simulator runtime. The carlos simulator is an iPhone 17 Pro.
- About 72 GB free. Old test results from other projects fill the home
  directory; do not delete them.
- Rust 1.93.0 with targets `aarch64-apple-ios`, `aarch64-apple-ios-sim` and
  `x86_64-apple-ios`. `cargo` is in `~/.cargo/bin`, which a non-interactive SSH
  shell does not put on `PATH`. Carlos now uses the separate1.100.0 toolchain.
- Three signing identities exist (two development, one distribution).
- No interactive Apple account is configured in Xcode.

## Rules

1. **Simulator first.** Run ordinary checks in the carlos simulator. Use a
   physical iPhone only for a check that needs its hardware, and say why first.
   Do not ask the owner to unlock a phone for a check the simulator can do.
2. **Own simulator.** Never boot, erase or install into another project's
   simulator. Never shut down a simulator that carlos did not start.
3. **Own directory.** Write only under `~/carlos-ios-build` on the Mac.
4. **No `rsync --delete` into the build directory.** Xcode keeps generated
   project files there; deleting them breaks the next build.
5. **`ControlMaster=no` on every SSH call.** A stale shared SSH connection
   makes `rsync` and `xcodebuild` hang. The script sets it.
6. **Close standard input** for SSH commands that take only arguments.
   Otherwise the remote command can consume the caller's next input line.
7. **Simulator builds need no keychain.** Sign with
   `CODE_SIGNING_ALLOWED=YES CODE_SIGN_IDENTITY=-` when the app uses the
   Keychain; an unsigned simulator build fails on its first Keychain write.
8. **Signed builds over SSH** need the login keychain unlocked in the same SSH
   session (`security unlock-keychain`, then `security set-key-partition-list`).
   Send the password through SSH standard input. Never put it in a command
   argument, a log or a file in this repository. `errSecInternalComponent`
   from `codesign` means the keychain is locked.
9. **Profiles.** When the app id gains a capability, a physical-device build
   must pass the App Store Connect API key to `xcodebuild` together with
   `-allowProvisioningUpdates`. Unlocking the keychain is not enough.
10. **Physical phone.** `xcodebuild test` waits until the phone is unlocked.
    Xcode 26.0.1 on this Mac may lack the developer image for a newer iOS
    version; check `xcrun devicectl device info details` before a device plan.

## Design consequences for the app

- The UI is Swift (SwiftUI). The app links the Rust crate `carlos-client`
  (see "Shared client core" in `docs/engine-architecture.md`) through Swift
  bindings that UniFFI generates. Swift owns the WebSocket and passes each
  message to the crate.
- The Mac compiles the crate. The Mac is an Intel machine, so the simulator
  needs the `x86_64-apple-ios` target and a phone needs `aarch64-apple-ios`.
  The build combines Intel and arm64 simulator libraries with lipo and packs
  that library plus the arm64 phone library into one XCFramework.
- A simulator on the Mac reaches a daemon on the development machine over the
  local network. This is the normal test setup: the daemon and its fixture
  peer run on Linux, the app runs in the simulator.
- Fonts (Inter, JetBrains Mono) must be bundled in the app; the Mac has no
  network account to fetch them at build time.
