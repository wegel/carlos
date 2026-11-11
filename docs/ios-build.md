# Building the iPhone app on the remote Mac

The iPhone app (ExecPlan 022 in `docs/engine-architecture.md`) compiles on a
remote Mac over SSH. The development machine runs Linux and cannot run Xcode.
The same Mac builds the owner's other iOS projects, so carlos keeps its own
directory and its own simulator there.

No app source exists yet. This note records the machine setup and the rules
learned on the other projects, so the first build does not repeat their
failures.

## Settings

`scripts/ios-mac.sh` reads `etc/mac-build.env`. Git ignores `etc/`, because the
repository is public and the file names a private host and key paths.

| Variable | Meaning |
|---|---|
| `MAC_BUILD_HOST` | `user@address` of the Mac |
| `MAC_SSH_KEY` | Path of the private SSH key for that Mac |
| `MAC_BUILD_DIR` | Directory under the Mac home that holds the carlos sources (`carlos-ios-build`) |
| `IOS_SIMULATOR_NAME`, `IOS_SIMULATOR_TYPE` | The simulator that carlos owns |
| `DEVELOPMENT_TEAM` | Apple team id for signed builds |
| `MAC_KEYCHAIN_ENV` | Local file (mode 0600) that defines `KEYCHAIN_PASSWORD` |
| `ASC_KEY_ID`, `ASC_KEY_FILE` | App Store Connect API key, for physical-device builds that must refresh a profile |

A new checkout needs a new `etc/mac-build.env`. Ask the owner for the values.

## Commands

    scripts/ios-mac.sh check          # macOS, Xcode, simulator, free disk
    scripts/ios-mac.sh ssh <command>  # one command on the Mac
    scripts/ios-mac.sh sim-create     # create the carlos simulator if missing
    scripts/ios-mac.sh sim-udid
    scripts/ios-mac.sh sync <dir>     # copy an app source directory to the Mac

Build, test, install and launch commands arrive with the app in ExecPlan 022.

## State of the Mac (checked 2026-11-11)

- Intel Mac, macOS 26.0.1, Xcode 26.0.1, Swift 6.2, `xcodegen` in `/usr/local/bin`.
- iOS 26.0 simulator runtime. The carlos simulator is an iPhone 17 Pro.
- About 72 GB free. Old test results from other projects fill the home
  directory; do not delete them.
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

- The app is pure Swift (SwiftUI). It speaks the WebSocket protocol from
  `docs/engine-architecture.md` and links no Rust code, so the Mac needs no
  Rust toolchain for carlos.
- Swift wire types come from the JSON Schema that `carlos-proto` emits.
- A simulator on the Mac reaches a daemon on the development machine over the
  local network. This is the normal test setup: the daemon and its fixture
  peer run on Linux, the app runs in the simulator.
- Fonts (Inter, JetBrains Mono) must be bundled in the app; the Mac has no
  network account to fetch them at build time.
