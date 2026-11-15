# EP019b: read and save host context-size defaults through the daemon

Commit `c9cfd9f` adds typed `settings/context/read` and `settings/context/set` commands for the existing per-backend context JSON files. The daemon uses its existing bounded workers for every file read and write. The socket manager and session owner never read those files themselves.

The read returns the requested session, its saved token count or Auto, and one explicit policy: owned, attached Codex, Claude shell override, or unsupported. A write requires a tagged choice, either `{"kind":"auto"}` or `{"kind":"tokens","tokens":500000}`. An omitted choice fails instead of silently choosing Auto. Tokens retain the existing inclusive 100,000 to 1,000,000 range and error text.

Attached Codex and unsupported Grok skip saved-file reads and refuse writes. Claude preserves the existing rule that any nonempty Unicode `CLAUDE_CODE_AUTO_COMPACT_WINDOW` value, including whitespace, blocks an explicit size but permits Auto. The saved default remains separate from live token usage. Saving does not mutate, restart, or compact the current native conversation. A later owned Codex launch receives the existing context-window and 90 percent compact-limit arguments.

Clients receive private saved/rejected results through the existing numbered-operation path. Retrying an older operation after another client saves a newer size cannot rewrite that newer file. Reconnecting with the original private identity preserves the result without replaying the write. Each read retains its original mailbox permit until the bounded worker returns, including after the requesting client disconnects.

The prerequisite `4634d90` shares the existing atomic writer between runtime and context preferences. It writes one whole JSON file through a unique temporary file in the target directory, syncs it, and renames it over the destination. Valid file symlinks retain their link and publish at the canonical regular-file target; broken links and non-file targets fail without replacement. Tests write only in temporary directories. Existing filenames, schemas, load fallbacks, runtime save API, and context error messages remain unchanged.

## Checked source and actual daemon cases

The isolated daemon uses base `c1f4c9a` plus the exact 29-path context patch. Its frozen executable is `/tmp/carlos-ep019b/context-defaults-final-sized-carlosd`, SHA256 `9d47a15d900856164719f19d7b3c7071d12b17509982668e4b9d4e9868f1b6f1`.

Seven actual-daemon workflows pass:

- Two clients save different defaults; old retries and a private reconnect retain results without overwriting the newer choice or exposing another client's result.
- An owned Codex session keeps its current process and live token usage after Save; a second native process receives the saved launch flags.
- Attached Codex and Grok refuse writes and ignore a malformed saved file.
- Claude's shell override rejects an explicit size and permits Auto.
- Malformed files, out-of-range values, missing choices, and directory targets return explicit errors without changing prior content.
- A FIFO blocks a real file read while another client reads the manager/session and sends an ordinary prompt.
- Eight disconnected FIFO readers release the existing worker/reply allowance so later clients can read.

The same first workflow fails against the frozen pre-API daemon with the precise unknown-method response. The old executable is `/tmp/carlos-ep019b/startup-defaults-final-carlosd`, SHA256 `fb05f8bca046c7e7b3fa254c10dc8eb2b84f29af5d0255833664db6fc7c386e9`.

The standalone test runs with the existing Python environment:

```bash
target/ui-venv/bin/python tests/daemon/test_context_defaults.py \
  --binary /tmp/carlos-ep019b/context-defaults-final-sized-carlosd \
  --artifacts /tmp/carlos-ep019b/context-defaults-repeat
```

Eleven focused proto/client/core cases, default and no-default workspace tests, and strict workspace Clippy pass. The final size-only helper extraction was checked again with default workspace tests, strict Clippy, all seven daemon workflows, and the normal integrated commit hook. Both preference suites pass. Exact source hashes, rejected runs, logs, and limits live in `/tmp/carlos-ep019b/context-defaults-handoff.json` and `context-defaults-identities.json`.

## Captured Claude child environment

Commit `4fe05f5` adds `SessionState::configure_context_shell_override(effective: Option<&str>)`. The lifecycle worker passes the child's resolved environment value before starting the session owner. A known absent or empty value permits a saved explicit size; a known nonempty value preserves the shell override. Context jobs capture that boolean, so they do not mistake the daemon's environment for an explicitly different child's environment. The raw value stays out of client facts. Legacy callers that have no captured child value retain the existing worker-side environment fallback.

Three new local tests cover absent, empty, whitespace, and Unicode values; both directions of child-versus-daemon policy; and exactly one inherited read when the child value is unknown. They mutate no process environment. All ten focused core context cases, strict core/all-target Clippy, and the normal integrated workspace-test commit hook pass. Logs are `/tmp/carlos-ep019b/context-child-env-tests.log`, `context-child-env-clippy.log`, and `context-child-env-commit.log`. The launch caller and actual per-launch option workflows belong to the separate startup slice.

## Limits and corrected discovery

An early inspection suggested a disconnected-read accounting leak, but the manager already releases those reservations when it removes a closed peer. No cleanup fix was needed or made; the eight-round case confirms the existing behavior. The first context commit attempt stopped before tests because a separate in-progress module referenced a missing test file; the retry passed after its author completed that module.

The terminal popup, final shared runner, and whole-EP checks remain root-owned. These checks use deterministic native peers without model inference. No reviewer or Fable ran under the owner's instruction to defer reviews until the end of EP019b. No executable was installed, merged, or pushed, and this report does not mark EP019b complete.
