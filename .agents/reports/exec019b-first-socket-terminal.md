# EP019b first actual socket terminal consumer

The opt-in command `carlos --daemon-socket PATH resume LOGICAL_SESSION_ID` now uses one pure Client and the existing terminal ViewState. This route creates no native backend or SessionState in the terminal. It reads/writes bounded nonblocking Unix frames and keeps a live editor/cached facts after disconnect. F5 reconnects with the same private greeting rights, restores checked facts and queries uncertain operations; it never repeats a mutation automatically.

The terminal lends client rows and complete source strings to the existing lazy render cache. Indexed row access keeps changed-row invalidation and visible-block reads constant time. The existing composer and simple approval painter accept borrowed data from either caller. The old core wrappers remain until the complete parity/deletion gate. No second full Message transcript or per-draw joined source body enters the socket route. Source failures report retired local tokens; this caller remembers the failed revision rather than reopening it on each append. The retained read map stays within 32 entries.

Private accepted prompts stay until their actual native clientId echo binds a shared row. The terminal then forgets handled records, keeping the 128-operation allowance available. A definite failure restores submitted text only into an empty own composer; it retains the record if newer text occupies that editor. An unconfirmed result retains text and never submits it again. Ordinary approvals use only offered y/n/s/c choices, preserve the composer and require the existing default-no second consent. Structured cards remain unfinished.

## Working checks

Two actual terminals attach to one actual daemon and deterministic native adapter. One answers a live card and interrupts the held native send while the second keeps its unsent Unicode draft. The byte recorder saves both sides of actual client/daemon JSON-RPC and native requests. A second workflow restarts the daemon, restores the same logical session through F5, preserves the draft and proves no send before Enter. The first full gate passes 49 daemon workflows and 185 terminal workflows, plus 67 helper and 15 architecture checks. Default workspace tests, strict lint and no-default workspace tests pass. Source logs are /tmp/carlos-ep019b/socket-ui-{terminal,tests,final-clippy,no-default,reconnect,four}.log.

A 130-send workflow supplies actual native clientId echo facts, then checks more than 128 handled operations and unique operation counters. An initial fixture omitted native user facts, which correctly kept its unbound private echoes; the fixture now emits the native identity required by the tested workflow. A source-pressure workflow publishes 9.5MiB before attaching a source watch, then checks that a local capacity failure stops that read across three subsequent native appends while the backend and composer remain usable. Generating the whole body behind an unread live watch instead exercised the daemon's independent outbox reset, so the strengthened fixture isolates local cache failure. These tests pass in socket-ui-limits-fixed.log and socket-ui-four.log. They do not establish larger-than-cache rendering parity.

The release `carlos perf-session --synthetic --turns 1000 --seed 1 --tool-lines 24` smoke preserves6001 messages,39347 rendered lines and6000 relevant items, with scroll/typing/append p95 .82/.61/.66ms in this one run. These are embedded-path checkpoint figures; the built-in RSS field is still unavailable and they are no matched final socket latency/RSS comparison. No release was installed.

## Engineering review

The first ephemeral review timed out after180sec without a verdict. The required stricter retry returned the following concrete finding:

1. Reviewed the new remote modules, direct client/render seams, terminal tests, and saved test logs. The logs confirm the reported checks passed; I did not rerun the full suite.

2. **MAJOR: daemon-reported source errors bypass failed-read bookkeeping.** In [client_replies.rs](/var/home/wegel/work/perso/carlos-daemon/crates/carlos-client/src/client_replies.rs:211), the client closes a failed watch without adding its token to `effects.bodies`. The terminal therefore leaves `Read.failed` false. On the next append to that entry, [bodies.rs](/var/home/wegel/work/perso/carlos-daemon/src/app/remote/bodies.rs:41) sees the missing body and opens another watch for the same failed revision. Repeated appends repeat the failed reads. The passing capacity test exercises a different branch, which does emit the retired token.

3. Emit the retired token when a watched source request returns an RPC error. Add a test that returns that error, appends three times within the same revision, and asserts that the terminal sends no further source subscriptions while its composer remains usable.

4. **FAIL**. Resolve the incomplete source-failure handling before proceeding.

Codex assessment: accepted the MAJOR finding. The failed watched-RPC branch now emits its retired token, just as local capacity/live notification failures do. A deterministic Unix fault recorder rejects the first source subscription; all other messages still use the actual daemon. The terminal observes three real native appends to that same revision and sends no additional source subscription, while its composer stays usable. Initial test setup tried to append to a completed history row; core correctly ignored that late delta, so the proof now starts an actual running source. Removing the retired-token line makes the actual terminal proof fail with three source subscriptions instead of two (socket-rpc-negative.log). The source is restored; final checks and a narrow engineering follow-up remain pending. No approval has occurred.

## Remaining scope

This is the first consumer checkpoint, not complete terminal parity or EP approval. The explicit route currently attaches an existing logical session and admits at most 32 document reads under one eight-MiB data allowance. Complete source/cache eviction, above-cache fidelity, copy/selection/navigation, history, structured questions, local commands, startup/create/discovery and the native panels/settings remain open. Unknown or lagging native prompt identities must remain visibly unconfirmed rather than be discarded or compared by text. The default path still embeds the core. Complete every workflow and then make the separate buildable embedded-path deletion commit before any EP auto-approval. A fresh Fable 5.2 medium whole-roadmap review is required at that final gate.


## Corrected engineering follow-up

1. Reviewed the failed watched-RPC branch, relay, focused UI test, and saved positive and negative evidence.

2. No remaining BLOCKER, MAJOR, or MINOR findings within this follow-up. The added token marks the terminal’s read as failed after the client closes its watch. Saved traffic confirms three appends to the same entry at revision `0`, two total source subscriptions, and a usable composer. Removing the fix produces the reported third subscription.

3. No further correction required for the prior MAJOR finding. I did not rerun tests or assess full EP parity.

4. **PASS**.

The source is restored after the negative proof. The restored final gate passes:49 daemon,189 terminal,67 helper and 15 architecture checks at target/ui-artifacts/run-CGmxBX. Six terminal workflows use the actual daemon socket: held-send controls/two drafts, restart/F5,130 bound private operations, local capacity failure, daemon-RPC failure and full large ANSI source. Workspace tests and restored strict lint pass; no-default workspace tests had already passed this source seam. Logs: /tmp/carlos-ep019b/socket-ui-final-terminal.log, socket-ui-restored-clippy.log and socket-ui-no-default.log.

The large ANSI workflow borrows456,047 exact source bytes from seven chunks, each at most64KiB, and checks the visible tail color cd0000. This warm attach took81.65ms, with terminal RSS27,368KiB and daemon RSS21,296KiB in that one debug test. It includes the recorder and existing saved history; it is no matched release comparison or bound on every body. Exact source comparison includes the existing command heading/footer. The terminal's selection/clipboard port still remains open.

Release identities at this checkpoint: carlos SHA2563bc054a3a4a625bdf98f18f8956d0741344292d8a7dfda8ffabd0e18e6ee2560; carlosd090b40b6dbbcf6c1fecc18a1721a8d25df8e6f96c69d6b6c512416a5487651ec. Main remains a29b1a1e3720e48b09e2ca984fb16c29b5857ea3, and installed Carlos remains d4595e2ac79f9ccb4cf279f08fc5dfc903e98aaec21960d7d0dc6c4727f6b2a3. No binary was installed or EP approved.


## Release evidence correction discovered during the local-view checkpoint

The plain scripts/test.sh gate builds debug executables. Its package step stages the same layout used by releases, but that label does not mean the staged binary was compiled in release mode. The3bc054a3 release executable identified above was built before the first socket consumer. The earlier perf-session smoke therefore checked that older embedded release, not the a3a7d4c consumer source. Keep its exact identity and raw logs, but do not treat those figures as a matched current-source performance gate. The191-workflow local-view checkpoint exposed the mismatch through binary sizes and debug_info. Codex is now building a3a7d4c in an isolated detached worktree and the current local-view source with matching cargo build --release flags, then comparing those actual releases with the original and EP019a baselines. No installed binary changed.
