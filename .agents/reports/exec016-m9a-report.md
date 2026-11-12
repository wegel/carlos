# Milestone 9.1: native settings and local settings drafts

The session owns acknowledged future settings, revision, accepted deferred work, pending native scope tokens, live-turn labels, error data and the legacy API fallback. The view keeps editor errors, rejected choices and the not-yet-submitted save preference. Once the user submits a valid request, its save/announce flags move to the session; a pending native request captures them exactly once. No app caller receives mutable settings facts.

The session returns ordered SettingsEffect values for local work: clear rejected choices, offer rejected values, save defaults or append the original notice. The app applies them once in order. In particular, a defaults-file failure still appears before the final native acknowledgment notice. The session does not import the client preferences module or any widget.

## Handler audit

Save still checks the advertised catalog/effort and allowed summary strings before retaining work; an invalid choice rejects only matching pending overrides and preserves existing valid native choices. Successful save clears old rejection, retains the latest deferred values and announces the change. Tick retains enabled/closed/pending checks, takes accepted deferred work once, allocates the same shared saturated token and captures the current turn. Future scope sends before live scope, with the same epoch/token/operation context and exact params. A send failure records the same rejection and live/future messages; it does not pretend that in-flight model work changed.

Native settings notifications still require the current root and an object. They advance revision, disable the legacy fallback, retain unknown native fields, clear pending runtime choices only when no deferred request exists, and apply the same future values.

Replies still require root, epoch, settings operation, exact token and params. Future/live scopes settle independently once. An older acknowledgment cannot overwrite a newer native notification. A precise unsupported-method error can use the existing direct-next-turn fallback only at the original revision; ordinary policy/validation errors retain native error data and rejected editor values. Save defaults only after acceptance or that exact fallback, once. A newer deferred request keeps its own editor values when an older request fails.

Close retains the same deferred-first/take-pending order, rejects the uncertain choices, and clears pending work without retry. Starting a turn captures the same model/effort/summary label. Reconnect installs authoritative choices with the same fields reset; a root switch resets settings while retaining only the established legacy-API fallback. Startup choice parsing and disk paths/formats remain unchanged.

## Checks and direct review

All 996 Rust tests pass, including all existing native settings, rejection, startup, runtime parity and reconnect fixtures. Two new session-only tests assert exact future-scope params, one save effect before the final notice, no second save on duplicate acknowledgment, and newer-native-event precedence. All 174 terminal workflows pass in 77.869 seconds. Release build, format, all-file style, whitespace and function/file size checks pass; the existing protocol_writer atomic rename remains the sole warning.

The primary agent directly compared every moved guard, default, ignored reply and scope-order rule with parent `0202c33`, using the engineering reviewer prompt. Verdict: PASS, no BLOCKER or MAJOR. No subagent or independent reviewer ran. The live-event and speed checks are recorded in the ExecPlan; artifacts stay under `/tmp/carlos-m9a/`.

The new SettingsFlags fields hold already accepted work, never an unsent checkbox. Copying the save preference at valid submit time avoids leaving accepted side effects only in a terminal. The old sticky true flags are preserved across deferred replacement until the request starts; invalid submissions retain their local unsent preference.

Additional caller changes only replace settings paths or call named startup/reconnect commands, including the model-settings keyboard adapter. No native protocol method, schema, dependency or installed binary changed.
