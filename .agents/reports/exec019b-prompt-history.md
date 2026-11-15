# Read and edit earlier prompts through the daemon

The terminal now recalls complete original prompts through `prompt/select` and
`prompt/text`. Each terminal keeps its own position, unsent composer and selected
human or unknown-source prompt. The daemon serves immutable prompt identities
and UTF-8-safe pieces of at most64 KiB. A prompt can remain editable before its
native echo arrives, and a rejected steer removes only its own unbound record.

The terminal installs a selected prompt only after every original byte arrives.
Typing cancels the pending read; rapid arrow keys keep their order under a
bounded queue. Rewind loads older transcript pages until the chosen prompt can
be shown. Escape restores the original composer. Claude requires an explicit
choice before editing a prompt whose source is unknown. Local display commands
still work during rewind, while other slash-prefixed text remains literal.

An edit result carries the private `rewound` fact. A failed fork keeps its target
so explicit retry forks again. A failed send after a successful fork keeps the
edited text but retires that target, so retry sends on the adopted branch. Newer
composer text stays untouched; F5 recovers retained operation-owned drafts into
an empty composer. Daemon replacement retires old prompt identities and requires
a new explicit choice. Rename uses its own private result and bounded recovery;
help consumes its keys and paste without sending hidden composer text.

## Checks and exact limits

The read foundation is commit `be5f883`, isolated tree and patch recorded under
`/tmp/carlos-ep019b/prompt-index-*`. It passed1515 Rust tests with10 ignored,
strict workspace Clippy,132 actual-daemon workflows, architecture/dependency,
compiled helper, package and installed Codex checks. The terminal run passed281
of283 cases; its two queue/card failures were repaired separately in `3d0c65a`.
The remainder passed two selection, five source-failure, two dictation and23
native-monitor cases. This was still a mixed embedded/socket terminal gate.

The terminal consumer passed four new actual-daemon cases, plus repeated Codex
and Claude edits, exact resumed UUIDs, a failed replacement, local Claude schema
validation, unknown-source choice and malformed context-file repair. Logs are
`history-ui-focused-second.log` (nine pass, two initial failures) and
`history-ui-focused-third.log` (both repaired cases pass) under the same temporary
directory. The first new case also requires the selected original to appear in
both the transcript and composer, proving older-page reveal. Frozen dev pairs
and SHA256 identities are in `history-ui-main-{second,third}-pair/identity.json`.
These pairs include coherent concurrent worker changes and are labelled as mixed
worktrees. They are not release or exact isolated-commit claims.

The help resize case now waits for the narrow help frame before sending End.
The Unix crossterm event reader can delay a simultaneous key after SIGWINCH;
the unchanged executable passes with that causal frame fence. The runtime keeps
the existing bounded End calculation. The context test explicitly expects the
read error after it deliberately replaces a settings file with a directory.
The structured Claude answer test now proves invalid17 never reaches the SDK,
then checks the exact corrected numeric23 answer. No screen/native assertions
were removed to hide a runtime failure.

The complete existing terminal suite is now running through actual private
daemons against the frozen third pair. Early failures identify additional cases
that sent keys before an accepted snapshot or completed settings read; those
still need individual checks and explicit readiness fences. Closed-Claude
explicit reopen, metrics, the old runtime deletion and final whole-EP release,
performance and review gates remain open. No reviewer, auto-approval, install,
main change, merge or push belongs to this checkpoint.
