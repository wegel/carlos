# EP019b: keep voice input on the terminal

Commits `e6417f5` and `bce1be1` let the daemon-backed terminal use the existing local dictation controls. The terminal owns its profiles, microphone, speech worker and unsent text. Core, the protocol and the shared client gain no audio dependency. Both the embedded wrapper and socket adapter use the same view methods until the embedded path is deleted.

The terminal handles queued keys before microphone events. Escape can cancel a capture before a late final result arrives. Each recording has a capture id; each transcription has its existing request id and cancellation token. Old auto-stop, error and transcription events cannot alter a replacement recording. Closing the view cancels its capture and worker token. Inserted text keeps the old history, Escape and rewind-draft behavior and stays private to that terminal.

Default and no-default workspace tests and strict Clippy passed. Focused tests passed 60 cases with one ignored in the default build and 16 cases without default features. Two actual-terminal workflows passed in each build. They cover missing-model profiles, command-line profile choice, F7/F9, local drafts during a busy native turn and absence of a chat send. The standard test script now runs the default pair; the standalone runner also accepts `--without-feature` for the no-default binary.

The measured default executable has SHA256 `b98288d11dd61612335601812447d3717d2aa01bfd39e09a0991d93d03b596f3`; the no-default executable has SHA256 `0bb0079479645db8a722162a6c2440d4a54bb16a37474c23d1efc022489d20df`. The exact source manifest, commands and logs are in `/tmp/carlos-ep019b/dictation-handoff.json` and `/tmp/carlos-ep019b/dictation-capture-manifest.json`.

The terminal cases deliberately use missing models and open no microphone. Physical microphone capture, hardware endpoint timing and actual Whisper inference were not run; the existing integration test remains ignored. No test-only product command or synthetic microphone was added. Root still owes the final integrated gate and embedded-path deletion. No intermediate reviewer ran, and no executable was installed, merged or pushed.
