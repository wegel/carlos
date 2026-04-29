# Add in-process voice dictation to the carlos input box

This ExecPlan is a living document. The sections `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Maintain this document in accordance with `.agents/PLANS.md`.

## Purpose / Big Picture

After this change, a user can press the dictation key in `carlos`, speak a prompt into a close-talking microphone, watch partial transcription appear in the input buffer, stop recording by pressing the key again or by pausing long enough for voice activity detection, edit the final text, and press Enter to send it as the next turn. The same flow must work during normal prompt composition, while Ralph mode is blocked and waiting for a human answer, and while rewind mode is editing a replacement prompt.

The feature exists for prompt dictation, not general desktop speech control. It is optimized for a single local speaker dictating natural language mixed with technical English vocabulary such as model names, library names, code identifiers, `TypeScript`, `Rust`, `async`, and `git`. The primary daily-use language is Quebec French, but the implementation must not assume French. Each dictation profile names an explicit Whisper language code, and any language supported by Whisper should work if the user provides a compatible model and profile.

The feature is intentionally in-process. Carlos must not shell out to `whisper-cli`, `sox`, `nerd-dictation`, or a sidecar daemon. Microphone capture, voice activity detection, audio resampling, and Whisper inference live inside the `carlos` process, off the terminal render and event loop.

## Progress

- [x] (2026-04-29 15:53Z) Created this ExecPlan from `stt.md`, grounded it in the current source layout, and moved the previous active code-quality ExecPlan to `.agents/done/` as requested.
- [ ] Milestone 1: Add compile-time feature gates, configuration loading, profile selection, and documentation stubs without enabling microphone capture yet.
- [ ] Milestone 2: Add app-level dictation state, key handling, cancellation semantics, profile picker state, and rendering indicators using fake transcription events in tests.
- [ ] Milestone 3: Implement audio capture, resampling, voice activity detection, bounded recording, and state transitions without requiring a Whisper model in normal tests.
- [ ] Milestone 4: Implement the single Whisper worker thread, model loading, vocabulary priming, transcription events, and cancellation inside the Whisper abort callback.
- [ ] Milestone 5: Integrate runtime profile switching, final README guidance, manual smoke tests, release build, installation, and engineering review.

## Surprises & Discoveries

- Observation: The current app has clear entry points for this feature but no async runtime dependency. Terminal events and backend events are already multiplexed through `src/event.rs` and `src/app/input.rs` using standard channels.
  Evidence: `src/app/input.rs` calls `spawn_event_forwarders(server_events_rx)` and drains `UiEvent` values in the TUI loop; `Cargo.toml` has no `tokio` dependency.

- Observation: `Ctrl+D` does not currently appear in the app keymap, while `Ctrl+M`, `Ctrl+R`, `Ctrl+Y`, `Ctrl+L`, `Ctrl+P`, `F6`, and `F8` are already used.
  Evidence: `src/app/input_events.rs` handles those chords in `handle_global_toggle_keys` and `handle_normal_key`. A repository search did not find an existing `Ctrl+D` binding.

- Observation: Current text insertion already has the right editing boundary for dictation. `AppState::input_insert_text` inserts at the current textarea cursor and resets history navigation when not rewinding; rewind mode preserves its selected anchor while editing.
  Evidence: `src/app/state_input.rs` implements `input_insert_text`, `input_apply_key`, rewind entry/exit, and history navigation.

- Observation: The current status bar can accept another mode label, but its reserved label area is tight. The dictation indicator should be integrated with `draw_status_bar` deliberately so it does not overlap context usage, model settings, or the Ralph label on narrow terminals.
  Evidence: `src/app/render.rs` computes reserved cells for context, model, and `RALPH MODE` before drawing the separator/status line.

## Decision Log

- Decision: Use `Ctrl+D` as the start/stop dictation key unless implementation-time testing proves a terminal conflict in the supported environments.
  Rationale: `Ctrl+D` is available in the current keymap and easy to remember for dictation. Carlos runs in raw terminal mode, so the normal shell EOF meaning does not apply while the app is focused.
  Date/Author: 2026-04-29 / codex

- Decision: Use `Ctrl+Shift+D` for the dictation profile picker if crossterm reports that chord reliably; otherwise fall back to a nearby explicit key such as `F7` and record the fallback in this plan and README.
  Rationale: The source spec suggests `Ctrl+Shift+D`, but terminals vary in how they encode shifted control characters. The implementation must prefer a reliable app behavior over a theoretical key chord.
  Date/Author: 2026-04-29 / codex

- Decision: Keep dictation optional behind Cargo features, with `dictation` as the user-facing feature and backend-specific Whisper acceleration features passing through only when explicitly requested.
  Rationale: `cpal`, `whisper-rs`, and their native dependencies are expensive for users who do not need microphone dictation. `cargo build --no-default-features` must still produce a working `carlos` with no audio or Whisper libraries linked.
  Date/Author: 2026-04-29 / codex

- Decision: Model the implementation around a single reusable Whisper context and one transcription worker thread.
  Rationale: Whisper model loading takes seconds and models are large. Loading per press would make the feature unusable, and concurrent inference against one context is unnecessary and risky.
  Date/Author: 2026-04-29 / codex

## Outcomes & Retrospective

(To be filled as milestones complete.)

## Context and Orientation

Carlos is a terminal UI for Codex and Claude sessions. The app state type is `AppState` in `src/app/state.rs`. It already aggregates transcript state, input state, Ralph mode state, model settings state, approval state, viewport state, and performance metrics. Add dictation state beside those existing runtime states rather than hiding it inside rendering or input code.

Terminal input is routed through `src/app/input_events.rs`. The function `handle_terminal_event` receives crossterm events and dispatches key events to `handle_key_event`. Normal prompt editing eventually calls `AppState::input_apply_key` or `AppState::input_insert_text` in `src/app/state_input.rs`. Dictation text must enter through similarly narrow state methods so it composes with multi-line input, history navigation, and rewind mode.

The main TUI loop lives in `src/app/input.rs`. It receives terminal events and backend server lines through channels, draws frames by calling `render_main_view`, and avoids blocking while a backend turn is active. Dictation must feed its own events into this loop without blocking it. If a new event source is added, extend `src/event.rs` and the `UiEvent` flow rather than polling from rendering code.

Rendering lives primarily in `src/app/render.rs`. `draw_input_area` draws the prompt area and gutter. `draw_status_bar` draws the separator/status line that already reflects active turns, context usage, model settings, Ralph mode, and rewind mode. Dictation needs a visible indicator during recording and transcription, for example `DICTATING [profile name]` while listening and `TRANSCRIBING [profile name]` while the model runs. Use a color distinct from Ralph mode, such as cyan or yellow, and check narrow widths so labels do not collide.

CLI parsing and persisted defaults live in `src/app/cli.rs`. Add `--dictation-profile <name>` there and include it in `CliOptions`. Backend startup and app configuration live in `src/app/backend_setup.rs`; initialize dictation after common app setup so both Codex and Claude backends get identical dictation behavior. Documentation belongs in `README.md`; add the dictation feature description, keybinds, profile file example, vocabulary rules, and model guidance in the same change that exposes the feature.

Ralph mode is the autonomous loop controlled by `.agents/ralph-prompt.md`. When Ralph emits its blocked marker, the app disables Ralph automation and waits for a human prompt. Dictation must work in that waiting-for-user state because it is just another way to fill and submit the input buffer. Rewind mode is the UI state where the user edits a prior prompt before forking the backend thread; dictation must insert into that rewind input buffer using the same cursor and selection rules as typed text.

A dictation profile is the unit of configuration. It bundles a human-readable name, a GGML Whisper model path, a Whisper language code, and an optional vocabulary file. Profiles are loaded from `$XDG_CONFIG_HOME/carlos/dictation.toml`, or from `~/.config/carlos/dictation.toml` when `XDG_CONFIG_HOME` is not set. If the file is missing, Carlos behaves as if the following default configuration existed:

    default_profile = "en"

    [profiles.en]
    name = "English"
    model = "$XDG_CACHE_HOME/carlos/whisper-model.bin"
    language = "en"

The implementation should expand `~`, `$XDG_CONFIG_HOME`, `$XDG_CACHE_HOME`, and `$HOME` consistently with the existing runtime-defaults path logic. If the active profile's model file is missing or unreadable, Carlos must start normally. When the user tries to dictate or switch to that unusable profile, show a clear one-line TUI status message and leave other profiles available.

## Plan of Work

First, add the compile-time boundary. `Cargo.toml` should define a `dictation` feature and make the audio and Whisper dependencies optional. The default feature set may include `dictation`, but `cargo build --no-default-features` must build without linking `cpal`, `whisper-rs`, `webrtc-vad`, or `rubato`. At minimum use `cpal` for microphone capture, `whisper-rs` for Whisper inference, `webrtc-vad` for voice activity detection, and `rubato` for resampling to 16 kHz mono. Add pass-through features for Whisper GPU backends, at least CUDA and Vulkan when those exact `whisper-rs` feature names are confirmed during implementation. The CPU-only default build must work on a fresh Arch machine with no GPU SDK installed.

Next, add configuration support behind the feature. A new module such as `src/dictation/config.rs` should load profiles, resolve paths, read vocabulary files, and choose the active profile from `--dictation-profile`. Missing `dictation.toml` returns the hardcoded English default. Malformed TOML is a clear error. A CLI-selected undefined profile fails fast with a useful message. A profile whose model path does not exist is not a config parse error; it is an unusable profile that becomes a one-line TUI message when used.

Then add app-facing state. A new `DictationState` should live in `AppState` and represent at least `Disabled`, `Idle`, `Recording`, and `Transcribing { partial: String }`. Cancelled is a transition, not a state. Track the active profile name, profile display name, last error message, and whether a profile picker is open. Keep the public methods on `AppState` small: start recording, stop recording and request transcription, cancel dictation, apply partial text, commit final text, set dictation error, and switch profile.

Wire key handling before ordinary text input. Pressing `Ctrl+D` while idle starts recording if no backend turn is active. Pressing `Ctrl+D` while recording stops recording and transcribes the bounded audio buffer. Pressing `Ctrl+D` while transcribing cancels the current transcription and starts a new recording. Pressing `Esc` while recording or transcribing cancels dictation, clears any partial text, and returns to `Idle`. The feature must not start while a backend turn is actively running; match the existing gating used for keyboard input during a turn. Normal text entry, approval overlays, help overlays, and model settings should keep their current priority unless this plan records a later decision to change the order.

Add text insertion semantics carefully. Partials and final text are inserted at the current cursor position, replacing any active selection if the textarea exposes selection. If partials are shown, replace the previous partial in place rather than appending repeatedly. Only the final transcription is committed as real editable text. If the user was browsing input history and dictates, treat it as starting a new draft, just like typing a character. In rewind mode, dictation goes into the rewind buffer and keeps the same selected history anchor that typed edits keep today.

Implement microphone capture and voice activity detection after the UI state is testable with fake events. Capture audio with `cpal`, resample to Whisper's required 16 kHz mono f32 using `rubato` when the device is not already at 16 kHz, and run `webrtc-vad` over the appropriate frame size. Recording auto-stops after about 800 ms of silence and also auto-stops at about 30 seconds. The audio buffer must be bounded; do not allow an unbounded `Vec<f32>` to grow for a stuck recording.

Implement Whisper inference as one dedicated worker thread. Load the active profile's Whisper context once and reuse it. When switching profiles, cancel any in-flight dictation, load the new model in the background, then drop the old context so only one Whisper model is resident. Send transcription requests to the worker over a channel as `(audio, cancellation_token)` or an equivalent typed request. The worker sends typed events back to the UI loop, including partial and final text if partials are implemented, errors, load-complete notifications, and cancellation acknowledgement.

When constructing Whisper `FullParams`, use explicit language from the profile and never auto-detect. Set translation off, use no prior context, suppress blank and non-speech tokens, and disable timestamp output. For prompt dictation under roughly 30 seconds, prefer a single segment. Read the active profile vocabulary at profile load time. The vocabulary format is one term per line with `#` comments. Join terms into a comma-separated `initial_prompt`. If the file is missing or empty, use the built-in technical vocabulary list for French, English, and other languages: `claude`, `codex`, `carlos`, `ralph`, `execplan`, `refactor`, `async`, `await`, `regex`, `TypeScript`, `Rust`, `npm`, `git`, `commit`, `struct`, `enum`, `trait`. Check the current `whisper-rs` limit for `initial_prompt` during implementation and truncate from the end if needed.

Add the runtime profile picker. The picker should visually match the existing resume/session picker in `src/app/terminal_ui.rs` and `src/app/picker_render.rs` where practical, or use a simple inline list if reusing that picker would force unrelated refactoring. Selecting a profile loads the model in the background, updates the status and indicator label, and frees the old model once the new one is active. If dictation is in flight when the user switches, cancel it first.

Finally, update `README.md` and validation. The README needs a dictation section under features, a controls update with the chosen keybinds, a sample `dictation.toml` with at least `fr-qc` and `en`, vocabulary file rules, the `--dictation-profile` flag, and model download commands for recommended models from `https://huggingface.co/ggerganov/whisper.cpp`. Document `ggml-large-v3-turbo.bin` as the recommended multilingual model, `ggml-large-v3-turbo-q5_0.bin` as the smaller quantized laptop-friendly option, and English-only `.en` models as an option only for users who dictate exclusively in English. Do not recommend French-fine-tuned community variants for Quebec French; the source spec notes that the 2025 CommissionsQC benchmark found European-French fine-tunes underperform the multilingual base for Quebec French.

## Milestones

### Milestone 1: Feature gate, config, and docs skeleton

At the end of this milestone, the project builds with and without the `dictation` feature, the CLI can parse `--dictation-profile`, and profile configuration can be loaded and tested without audio hardware. This milestone should add optional dependencies in `Cargo.toml`, a small `src/dictation/` module tree guarded by `#[cfg(feature = "dictation")]`, and no microphone capture yet. Acceptance is `cargo test`, `cargo build --no-default-features`, and focused unit tests for missing config fallback, malformed TOML, undefined CLI-selected profiles, model-path usability status, and vocabulary parsing.

### Milestone 2: App state, key routing, rendering, and fake events

At the end of this milestone, the TUI can display dictation state and process fake dictation events in tests. Add `DictationState` to `AppState`, route `Ctrl+D`, route `Esc` cancellation, add the profile picker state, and render `DICTATING [name]` and `TRANSCRIBING [name]` without overlapping existing labels. Do not require a real mic or model for these tests. Acceptance is a unit test proving `Idle -> Recording -> Transcribing -> Idle`, tests for all cancellation paths, and render tests showing the indicators in normal mode, Ralph blocked waiting state, and rewind mode.

### Milestone 3: Microphone capture, VAD, bounded audio, and resampling

At the end of this milestone, pressing the dictation key can record from the default input device, stop on a second key press or after about 800 ms of silence, and hand a bounded 16 kHz mono f32 buffer to the transcription request path. The implementation must handle no input device, stream creation failure, and stream errors as one-line TUI errors. Acceptance is `cargo test --features dictation` with fake capture tests, plus a manual local smoke test that records audio and shows a transcribing state without blocking the UI.

### Milestone 4: Whisper worker, cancellation, vocabulary, and final text insertion

At the end of this milestone, a valid profile with a local model can transcribe a recorded utterance and insert final text into the input buffer at the cursor. The worker must be single-threaded, reuse one context, catch panics from inference defensively, and check cancellation inside Whisper's abort callback so `Esc` and repeated `Ctrl+D` do not allow ghost text to arrive late. Acceptance is unit coverage for partial replacement and final commit behavior, vocabulary prompt construction, cancellation dropping late events, and a manually ignored integration test using `ggml-tiny.bin` and a tiny audio fixture.

### Milestone 5: Profile switching, README, release validation, and review

At the end of this milestone, runtime profile switching works, README guidance is complete, and the release binary is rebuilt and installed. Acceptance is a French-profile manual smoke, an English-profile manual smoke, a missing-model smoke, `cargo test`, `cargo build --release --features dictation`, `cargo build --release --no-default-features`, `install -Dm755 target/release/carlos ~/.local/bin/carlos`, and an engineering reviewer verdict copied into this ExecPlan.

## Concrete Steps

Run commands from the repository root:

    cd /var/home/wegel/work/perso/carlos

Inspect the existing entry points before editing:

    sed -n '1,240p' Cargo.toml
    sed -n '1,240p' src/app/cli.rs
    sed -n '1,260p' src/app/state.rs
    sed -n '1,260p' src/app/state_input.rs
    sed -n '1,480p' src/app/input_events.rs
    sed -n '1,260p' src/app/input.rs
    sed -n '120,340p' src/app/render.rs
    sed -n '1,220p' README.md

Add optional dependencies and features in `Cargo.toml`. The exact crate versions and `whisper-rs` backend feature names must be confirmed at implementation time, but the shape should be:

    [features]
    default = ["dictation"]
    dictation = ["dep:cpal", "dep:rubato", "dep:serde", "dep:toml", "dep:webrtc-vad", "dep:whisper-rs"]
    dictation-cuda = ["dictation", "whisper-rs/<confirmed-cuda-feature>"]
    dictation-vulkan = ["dictation", "whisper-rs/<confirmed-vulkan-feature>"]

    [dependencies]
    cpal = { version = "...", optional = true }
    rubato = { version = "...", optional = true }
    serde = { version = "1", features = ["derive"], optional = true }
    toml = { version = "...", optional = true }
    webrtc-vad = { version = "...", optional = true }
    whisper-rs = { version = "...", optional = true }

Create the dictation module tree. The exact split may change, but start with these coherent files and keep each under the code-style limits:

    src/dictation/mod.rs
    src/dictation/config.rs
    src/dictation/state.rs
    src/dictation/vocabulary.rs
    src/dictation/worker.rs
    src/dictation/audio.rs
    src/dictation/vad.rs

Guard real audio and Whisper code with `#[cfg(feature = "dictation")]`. Provide no-op or disabled app-state behavior when the feature is not compiled so the rest of the app code can build cleanly with `--no-default-features`.

Extend `src/app/cli.rs`:

    Add `dictation_profile: Option<String>` to `CliOptions`.
    Parse `--dictation-profile <name>`.
    Include the flag in `usage()`.
    Add parser tests beside existing CLI tests in `src/tests/runtime_tests.rs` or the existing parse test module.

Extend `src/app/state.rs` and `src/app/state_input.rs`:

    Add a dictation field to `AppState`.
    Add small methods for start, stop, cancel, partial replace, final commit, profile picker open/close, and profile switching status.
    Ensure final text insertion uses the same input-history and rewind rules as `input_insert_text`.

Extend `src/event.rs` and `src/app/input.rs` if needed:

    Add typed dictation events to the UI event channel.
    Drain those events in the same loop that already drains terminal and backend events.
    Do not block `terminal.draw`, `handle_terminal_event`, or backend message processing while recording or transcribing.

Extend `src/app/input_events.rs`:

    Add the dictation start/stop key before normal text insertion.
    Add dictation cancellation to `Esc`.
    Preserve existing priority for approval, help, model settings, and quitting.
    Keep active-turn gating consistent with existing input behavior.

Extend `src/app/render.rs` and related rendering helpers:

    Draw a visible recording/transcribing indicator using the active profile display name.
    Use a color distinct from Ralph mode.
    Ensure narrow terminals truncate or drop labels gracefully rather than overlapping context usage or model labels.
    Add or update render tests in `src/tests/ui_render_tests.rs`.

Add tests as the implementation grows:

    cargo test
    cargo test --features dictation
    cargo build --no-default-features
    cargo build --release --features dictation
    cargo build --release --no-default-features

When installed runtime behavior changes, finish with:

    install -Dm755 target/release/carlos ~/.local/bin/carlos

## Validation and Acceptance

The feature is accepted when automated tests and manual behavior both prove it works.

The required automated checks are:

    cargo test
    cargo test --features dictation
    cargo build --release --features dictation
    cargo build --release --no-default-features

`cargo build --release --no-default-features` must produce a working `carlos` without audio or Whisper dependencies linked. A normal test run must not require microphone hardware or a downloaded Whisper model. Hardware and model tests should be gated behind the `dictation` feature and marked `#[ignore]` when they need `ggml-tiny.bin` or an audio fixture.

The required manual checks on a machine with a microphone and at least one model downloaded are:

1. Start `carlos` with a French profile selected, press the dictation key, say `ecris-moi un script en TypeScript qui parse du JSON`, stop recording, and confirm the input buffer contains the French sentence with the technical term spelled correctly.
2. Start `carlos` with an English profile selected, press the dictation key, say `write me a TypeScript script that parses JSON`, stop recording, and confirm the input buffer contains correct English text.
3. Press Enter after dictating and confirm the transcribed text is submitted as the user turn.
4. Switch profiles at runtime through the picker and confirm the indicator label changes and the next dictation uses the new profile language.
5. Press `Esc` while recording and while transcribing. In both cases, the input buffer returns to its previous text, partials disappear, and no late transcription text arrives.
6. Enter rewind mode, dictate replacement text, and submit. The dictated replacement should fork from the selected prior prompt just as typed replacement text does.
7. Reach a Ralph blocked state where the transcript says `@@BLOCKED@@`, dictate an answer, press Enter, and confirm the answer is submitted normally.
8. Delete or rename the active profile model file, attempt dictation, and confirm Carlos shows a clear one-line error while the rest of the app continues working.
9. Remove `dictation.toml` and leave no model at the default path. Carlos should start normally and report dictation unavailable only when the user tries to use it.

Because this changes installed runtime behavior, a successful implementation must rebuild the release binary and install it to `~/.local/bin/carlos`.

## Idempotence and Recovery

The configuration loader must be safe to rerun and must not create or download model files automatically. Users are responsible for downloading GGML models. If a profile points to a missing model, leave that profile in the profile list but mark it unusable on selection or use.

Recording cancellation must be idempotent. Pressing `Esc` repeatedly or receiving a late worker event after cancellation must not mutate the input buffer. Use monotonically increasing request ids, cancellation tokens, or an equivalent generation check so stale partial and final events are ignored.

Profile switching must be recoverable. If loading the new model fails, keep the old usable model active when possible, show the error in the TUI, and return to `Idle`. If there was no old usable model, return to `Disabled` with a clear status. Do not keep multiple Whisper contexts resident after a failed or successful switch.

If `cpal` reports a stream error mid-recording, cancel the recording, drop buffered audio, show a one-line error, and return to `Idle`. If Whisper panics inside the worker, catch it with `std::panic::catch_unwind`, surface an error event, and keep the TUI alive.

## Artifacts and Notes

The source specification was `stt.md`. Its hard constraints are incorporated here so this ExecPlan remains self-contained:

- In-process only. Do not shell out to external speech tools.
- No new long-running OS processes. Use app tasks or threads.
- Optional at compile time. `--no-default-features` must build without audio or Whisper.
- Model files are not bundled or auto-downloaded.
- Never block the UI thread for capture, VAD, resampling, model loading, or inference.
- Dictation is cancellable, and late ghost text must not arrive after cancellation.
- Use `cpal`, `whisper-rs`, `webrtc-vad`, and `rubato` unless a later implementation-time decision records and justifies a replacement.

Sample user configuration for README and tests:

    default_profile = "fr-qc"

    [profiles.fr-qc]
    name = "Quebecois"
    model = "~/.cache/carlos/ggml-large-v3-turbo.bin"
    language = "fr"
    vocabulary = "~/.config/carlos/vocab-fr.txt"

    [profiles.en]
    name = "English"
    model = "~/.cache/carlos/ggml-large-v3-turbo-q5_0.bin"
    language = "en"
    vocabulary = "~/.config/carlos/vocab-en.txt"

Vocabulary file format:

    # One term per line. Comments begin with '#'.
    claude
    codex
    carlos
    ralph
    execplan
    TypeScript
    Rust

Recommended README model guidance:

    ggml-large-v3-turbo.bin       about 1.6 GB, multilingual, recommended default for non-English users.
    ggml-large-v3-turbo-q5_0.bin  about 550 MB, multilingual, quantized, good CPU-only laptop default.
    ggml-medium.en.bin and other .en models are English-only options for users who dictate only English.

## Interfaces and Dependencies

At the end of the implementation, there should be an app-facing dictation API shaped roughly like this, with names adjusted only if the local module style suggests clearer names:

    pub enum DictationState {
        Disabled,
        Idle,
        Recording,
        Transcribing { partial: String },
    }

    pub struct DictationProfile {
        pub id: String,
        pub name: String,
        pub model: PathBuf,
        pub language: String,
        pub vocabulary: Option<PathBuf>,
    }

    pub enum DictationEvent {
        RecordingStarted,
        RecordingStopped,
        Partial { request_id: u64, text: String },
        Final { request_id: u64, text: String },
        Cancelled { request_id: u64 },
        ProfileLoaded { profile_id: String },
        Error { message: String },
    }

    pub enum DictationCommand {
        StartRecording { profile_id: String },
        StopRecording,
        Cancel { request_id: u64 },
        SwitchProfile { profile_id: String },
        Transcribe { request_id: u64, audio_16khz_mono: Vec<f32> },
    }

The exact channel type is up to the implementer, but commands and events must be typed. Avoid encoding worker protocol as raw strings or JSON inside the app.

Whisper inference parameters must enforce:

    language = Some(profile.language)
    translate = false
    no_context = true
    single_segment = true for normal prompt-length utterances
    initial_prompt = Some(joined vocabulary) when available
    suppress_blank = true
    suppress_non_speech_tokens = true
    print_timestamps = false
    token_timestamps = false

## Revision Notes

2026-04-29 / codex: Created this ExecPlan from `stt.md` and repository inspection so Ralph or a fresh agent can implement dictation without needing the untracked source note. The previous active ExecPlan was moved to done because the user explicitly requested that any previously active EP be moved to done.
