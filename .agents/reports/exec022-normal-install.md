# EP022 normal local install handover

The owner requests making the branch their normal local Carlos on2026-11-17.
The runtime is checked source92d3a53538102e52153236eb9c0b4ba0, from the complete
release/native gate04. A fresh cargo build --release is current. All515 saved
package hashes match. The fresh standalone package smoke passes, including the
matching daemon, hostile working directory, explicit environment and helper
self-worker checks. These checks use isolated fixtures, not owner sessions.

The install has not happened. The tool container mounts~/.local/bin read-only;
both the ordinary and escalated attempts fail with EROFS before swapping a file.
The container's unmounted home paths are private too, so the handover uses the
shared checkout. It contains the full package with notices/checksums and the old
CLI/helper backups under tmp/install-carlos-92d3a535/. Its install.py --check passes.
The user runs this from their own normal shell, without sudo:

    python3 /home/wegel/work/perso/carlos-daemon/tmp/install-carlos-92d3a535/install.py

The script verifies package/old-file hashes, saves the package and a backup receipt
under the real~/.local/lib/carlos, stages regular executables in~/.local/bin, then
renames helper and daemon before CLI and verifies both Rust build identities. It
starts or stops no daemon. The old CLI hash is
d4595e2ac79f9ccb4cf279f08fc5dfc903e98aaec21960d7d0dc6c4727f6b2a3;
the old helper hash is3cfbb394b48a620034243e0f9039ab6c329bf07ae3577c26b05a713263a0b612.
No carlosd was previously installed beside them.

Plain Carlos starts/connects the default managed local daemon. The current iPhone
uses a separately started daemon/socket/catalogue in the preview directory. An
async question asks the owner whether they want to share it by default or retain
explicit attachment. That preference remains unanswered. Do not confuse installing
the matching files with moving a live native owner or migrating the catalogue.

The iPhone18 Pro remains on checked de74, which the owner calls awesome after
trying its native scrolling. No installed phone/daemon was changed by the failed
CLI install attempt. EP022's final cost/sweep/review work is still incomplete.
