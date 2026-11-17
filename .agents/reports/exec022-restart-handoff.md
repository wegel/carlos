# EP022 sandbox restart checkpoint

Root stops at the owner's explicit request so the sandbox can gain writable access
to the actual Carlos files. Resume only EP022, directly, without developer agents
or intermediate reviewers. Worktree:/var/home/wegel/work/perso/carlos-daemon,
branch iphone-spike. The default cwd may be the protected main checkout instead.
Read AGENTS.md, .agents/CODE_STYLE.md, PROGRAM_PLAN.md and the active EP022 first.

## State to preserve

The phone remains on signed de74af3e4ef7529f93d6. The owner tested it and said the
scrolling is awesome. Its native list replaces flow/manual gesture correction;
normal and strong flicks match plain iOS within3% on the private conversation and
within1% on generated history. Keep that scrolling behavior. Do not restart the
preview phone daemon, which the owner runs outside the tool container on port33731.
Its socket/catalogue are in the shared checkout under tmp/phone-preview-b7744b8.
No phone recording was started; the temporary recorder/hook were removed.

The normal-shell CLI install is now visible and verified. Installed source is
92d3a53538102e52153236eb9c0b4ba0. Hashes are CLI7167fb9c96405436af5cfce1051dfc10619aef3d0cda468f97a8476cdc202e74,
daemonfefa88341566f89100d924abf14e48bd3e6070ef64438f86c7952a663a49d482,
helper9b201307f67de29cadabdabc3f9a4df5ddd736bc5b4d145c0b6d3831dc2a8ad3.
The owner runs the new auto-started normal daemon and says it is idle. It is a
separate catalogue from the preview and currently has no WebSocket listener.
Installing files did not restart either daemon. Previous no-install scope is
superseded only for the explicitly requested normal install. Keep main, archived
plans and all frozen previews unchanged; no push/merge/publication is requested.

## Mounts and user files

The old sandbox bound~/.local/bin read-only. An escalated retry still returned
EROFS. Unmounted home directories were private overlay files, not the user's
actual home. Do not trust previously created overlay~/.local/lib files. The old
namespace maps UID0 to the host user's1000; do not chown shared files to namespace
UID1000. Shared work files appear as UID0 inside the sandbox.

The owner was asked to expose real~/.local/bin,~/.local/lib/carlos,
${XDG_CONFIG_HOME:-$HOME/.config}/carlos,
${XDG_STATE_HOME:-$HOME/.local/state}/carlos, and
$XDG_RUNTIME_DIR/carlos (normally/run/user/1000/carlos). Mount the socket parent
and SQLite state directories, not individual socket/DB files. Carry through the
real XDG runtime/config/state environment values. Keep the checkout shared.
Do not accidentally launch a normal daemon from the tool container: use an
explicit known --daemon-socket for tool reads. The normal user should own its
server process. No network setup/restart helper has been written yet.

The verified package and original installed CLI/helper backups survive in the
shared tmp/install-carlos-92d3a535/. Its installer has already served its purpose;
do not rerun it blindly because it checks old hashes. The normal-shell installer
writes its receipt/backups under the real~/.local/lib/carlos/backups/20261117T191240Z.
Inspect that actual directory after mounting it. The full release/native gate04
and fresh package smoke pass; runtime source has not changed since those checks.

## New saved-server work

The owner sees two Alpha entries, cannot discover edit/remove, and wants an entry
for the normal daemon. Current installed app adds through the Sessions sheet's
old bottom Connect another host button. New source23952a4b8d31245275a0 moves Add
server to the toolbar and shows each server's name/address plus a visible Manage
menu with Edit connection, Reconnect and Remove from iPhone. Removal confirms the
specific name/address, removes only its saved token/local preference, and leaves
native work running. Duplicate labels display addresses in the host picker too.

Address edits now keep the stable saved ID. A genuinely changed endpoint uses a
fresh model/transport and explicit token; only its authenticated attempt replaces
the original. Failed/cancelled edits keep the old entry. Equivalent addresses
cannot add another record. Stale editors cannot resurrect removed entries. A model
replacement resets its session view by object identity. Production code compiles;
24 native tests and1795 Rust tests (12 ignored) pass.

Source713e524dd1050306d293 ran the new actual UI checks. Both reached removal and
failed only because XCTest matched two nested views for the Remove button. The
query now uses host.confirmRemove.firstMatch. That selector-only fix is frozen
in23952 but has NOT been rerun. No new server-manager app has been deployed.
Logs:/tmp/carlos-ep022/hosts-native-01.log, hosts-cargo-01.log,
hosts-edit-ui-01.log, hosts-duplicates-ui-01.log. Failed xcresults are on the Mac:
713e524dd1050306d293/ui-tests-20261117T144556.xcresult and
713e524dd1050306d293/ui-tests-20261117T144854.xcresult.

After restoring private artifacts, run the following on23952, retaining180s
limits and actual native traffic assertions:

    CARLOS_IOS_SOURCE=23952a4b8d31245275a0 target/ui-venv/bin/python scripts/ios-fixture.py --workflow hosts
    CARLOS_IOS_SOURCE=23952a4b8d31245275a0 target/ui-venv/bin/python scripts/ios-fixture.py --workflow host-duplicates

Then check credential rejection/relaunch/token-update before signing and deploying
the server-manager app. The two fixtures use two real loopback forwards to one
actual daemon and must create/stop no extra native owner. New test code is
apps/ios/UITests/HostWorkflowTests.swift and apps/ios/Tests/HostStoreTests.swift.
No code review agent has run; reviews still wait for the end of the whole EP.

## Normal daemon phone access

The owner answered: started automatically, and it is idle. Determine its actual
managed socket/state after the corrected mounts. Defaults are
$XDG_RUNTIME_DIR/carlos/daemon.sock, or state/carlos/daemon/run/daemon.sock when
XDG_RUNTIME_DIR is absent; state root is XDG_STATE_HOME or~/.local/state.
Phone access requires --listen on an explicit LAN IP and a device grant. Use a
separate port such as33732; preserve preview33731. The source accepts one unicast
IP, not0.0.0.0. The current LAN address used by the preview is192.168.3.11.

The auto launcher does not currently read listener settings. Enabling the current
listener requires a graceful restart with the SAME normal socket/catalogue, from
the user's normal shell. SIGTERM/SIGINT are handled gracefully; never use a broad
pkill that would stop the preview too. Prepare a concrete handover and address
how future starts retain the listener; do not claim plain Carlos already starts
a phone endpoint. Grant with carlosd device add --socket PATH --name NAME
--token-file NEW_PATH. Keep token files private; no token for this new normal
catalogue has been generated. Once reachable, the phone Add server form needs a
clear name, actual ws://LAN-IP:PORT, this daemon's token, and the explicit private-
network toggle. Do not reuse the preview token against another catalogue.

## Remaining EP work

A separate uncommitted memory probe39c0e5c6fadc2526a385 disables only native cell
prefetch. The exact10MB/exact-copy check passes47.51s; peak RSS447560KiB versus
1126612KiB on accepted de74, about60% lower. Logs no-prefetch-source-01/memory-01
and Mac source-memory-39c0...json retain proof. The working tree keeps normal
prefetch; do not replace the owner-accepted normal history behavior globally.
Consider limiting cell prefetch only for oversized original documents after the
server-management/normal-daemon work. No extra parser cache is justified by this
measurement. The older trace showed completed answers parsed once, then cache hits.

Final functional/cost checks, Linux frozen-pair timings, production counts, Root
engineering review and ONE end-only Fable5.2 medium whole-roadmap review remain.
No reviewer has run. The whole phone/web objective must inform that final review.
Do not start another EP. The current final-pair runtime is92d3a535; do not use the
older /tmp/carlos-ep022/final-pair without its suffix. Accepted baseline is
/tmp/carlos-ep021b/reviewed/final-pair. Timing scripts are
measure-matched-92d3-latency.py and measure-matched-92d3-sockets.py. Run timing only
without builds/tests/reviewers competing. Archive EP022 only after all checks and
end reviews pass under delegated approval; do not merge or publish.

## Private artifacts and build host

The restart archive is ignored shared tmp/ep022-restart-state/private-artifacts.tar.zst.
It preserves the whole /tmp/carlos-ep022, accepted EP021b final-pair and the
EP019a function-counter helper needed by measure-production.py. SHA256SUMS beside
the archive identifies it. Restore only these private /tmp prefixes if lost:

    tar --zstd -xf tmp/ep022-restart-state/private-artifacts.tar.zst -C /tmp

Do not overwrite surviving different artifacts. The archive includes private
captures/fixture credentials; never commit it or send it to reviewers. Shared
source snapshots and target/ios-artifacts already survive. The generated full
matrix runner was not restarted; the old9a45 launcher was stopped after its child
exited. No tests/builds/background fixture runs remain active at this checkpoint.

The owned Mac and key paths come from ignored etc/mac-build.env; its build root
is~/carlos-ios-build. Use the scripts and never print secrets. Every build/test/sign/install uses
flock target/ios-artifacts/mac-build.lock. The owned simulator is
4C821FE4-54D8-452D-A46C-3187AAED6C61. The exact iPhone18 Pro identifier is in the ignored
device-tools/install-current-preview.py; never touch the other paired iPhone12. Existing immutable signed previews, Mac
results and device-tools remain. Native deployment helpers live in the restored
/tmp/carlos-ep022/device-tools and the Mac device-tools directory. Existing build/
sign-native-list-de74-preview.sh show the checked signing pattern. Use new source-
specific names and immutable signed bundles for the next phone install.
