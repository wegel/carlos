# Carlos client protocol version 1

The `carlos-proto` crate defines the JSON that terminal, phone and browser clients share. It contains data types and checks, with no sockets, threads, clock reads or terminal dependencies. EP018 publishes these facts from the core and lets the terminal consume numbered records directly in memory. EP019 adds a server, command handlers, reconnect replay and the shared client state machine. EP020 adds WebSocket access, device tokens and generated JSON Schema. The types below do not imply that a socket server already exists.

## Envelopes and addresses

Every request carries `jsonrpc: "2.0"`, an `id`, a `method` and an object `params`. The id is a string or signed 64-bit integer; null and fractional request ids are invalid. Responses contain the same id and either `result` or `error`, never both. A failure to parse an envelope may return a null id. Notifications contain `jsonrpc`, `method` and `params` with no id. The Rust types are `Request`, `Success<T>`, `Failure` and `NotificationMessage`.

For example, a client greets a server with:

    {"jsonrpc":"2.0","id":"hello-1","method":"hello","params":{"protocol":1,"client":"carlos-ios","token":"device-token"}}

The server checks the token and calls `check_protocol` before accepting commands or subscriptions. Version 1 is the only accepted version. A mismatch returns Carlos error `-32001`. Standard JSON-RPC errors remain `-32700` for malformed JSON, `-32600` for an invalid envelope, `-32601` for an unknown method and `-32602` for invalid parameters. EP019 must return explicit Carlos errors for unknown sessions, retired entries, unavailable choices and answers to already resolved cards. Failed commands do not allocate entry ids or advance the session cursor merely because a client attempted them.

A session address contains 32 lowercase hexadecimal characters. An entry or approval address contains 32 lowercase hexadecimal characters, a hyphen, then 16 lowercase hexadecimal characters. Clients treat the whole string as opaque. They must not derive order, backend identity or row position from it. The parser rejects uppercase, short ids and non-ASCII characters. A syntactically valid id does not prove that the core ever issued it. A lookup must still find the live row or card. Native thread ids and backend request ids remain inside the core; ordinary user or backend prose can still mention an id as text.

A cursor (`seq`) is a canonical decimal **string**, including zero as `"0"`. Decimal strings preserve all 64 bits in JavaScript. Leading zeros, signs, numeric JSON values and values above `18446744073709551615` are invalid. Each session starts before event 1; the core advances its clock without wrapping. A native fork, history reload or transcript clear preserves the logical session address and clock while retiring the affected entry addresses.

## Client commands

All names and parameter keys are case-sensitive. Optional fields may be absent or null. Fields that the core knows to be absent are encoded as null, so a full replacement can clear old facts. An empty parameter list is encoded as `{}`.

- `hello`: `protocol` (integer), `client` (string), `token` (string). Returns `HelloResult` with `protocol`, `host`, `daemon_version`.
- `sessions/watch`: `{}`. Returns `Sessions` with `sessions`, then receives `sessions/changed` notifications.
- `session/create`: `backend` (`codex`, `claude`, `grok`), `cwd` (string), `name` (optional string). Returns `SessionInfo`.
- `session/subscribe`: `session`, optional `since` cursor. Returns `SubscribeResult`, either `kind: "snapshot"` with `snapshot`, or `kind: "replay"` with `events` and final `seq`. The server must establish this cursor before delivering later notifications for the subscription.
- `session/unsubscribe`: `session`. Returns null.
- `session/history`: `session`, `before` (live entry id), `limit` (1 through 1000). Returns `History` with `entries`, `has_older`, `seq`. Rows precede `before` strictly, in chronological order. A retired or foreign cursor fails; the server never guesses a replacement position.
- `session/send`: `session`, `text`, `client_message_id`. Returns `SendResult` with `entry`. The daemon will copy the client token into the resulting user entry so a client can match its immediate local echo. EP018 does not yet accept these remote commands; its existing local submissions have a null token. EP019 must define retry handling before it accepts sends.
- `session/interrupt`: `session`. Returns null. Backend notices describe what actually stopped; a successful request alone does not assert that the backend finished.
- `session/approve`: `session`, `approval` (live card id), `answer`. An answer is either `{"kind":"decision","decision":"once"}` or `{"kind":"input","value":...}`. Other decisions are `session`, `deny`, `cancel`. The handler must check the live card, its offered choices and submitted state before replying to the backend. Structured values must pass the card's question or form checks. EP019 adds the single winning answer rule for several clients.
- `session/setRalph`: `session`, `on` (boolean). Returns null. The core may defer a toggle until the current turn ends; the event reports the accepted state.
- `entry/output`: `session`, `entry`. Returns `OutputResult` with `lines` and `seq`. Each line has `kind` and `text`. The client discards a stale fetch if a later event changed that row's output.

The serializable command types describe the client boundary. They do not expose Codex JSON-RPC methods, Claude callback ids or terminal key names. The core continues to use its existing backend handlers in EP018.

For a `fields` input, the submitted `value` is an object with a `fields` array. Each answer has `key`, `values` (an array of the listed typed option values) and nullable `text` (free text or notes). Unknown or duplicate keys fail. A field with `multiple: false` accepts at most one listed value. Nonempty text requires `other: true`; `notes_only: true` requires a listed value alongside those notes. A required field cannot be empty. The core checks these rules before converting to a native backend answer. For a `form` input, `value` is the JSON value described by its schema, checked before a native reply. A browser input uses the explicit `once` decision. An unsupported input can be declined or cancelled, but cannot accept an invented empty answer. EP019 must implement these checks at the command handler; EP018 exposes the descriptions and keeps existing local editors.

## Session facts

`SessionInfo` contains `id`, nullable `name`, `backend`, nullable `model`, `host`, nullable `cwd`, `phase`, `ralph`, nullable `turn_started_at`, nullable `context` and nullable `quota`.

The core takes the actual backend from the launch caller. It does not guess from compatibility controls, which can be enabled together. The in-process terminal uses host `local`; the daemon will supply its host label. The core reports phase `approval` when an active approval card exists, otherwise `running` when a turn is active, otherwise `idle`. A queued Ralph toggle does not change `ralph` until the core applies it.

`turn_started_at` is the core's observed start time in Unix seconds, or null when unknown or idle. Clients update their own elapsed timers. A same-thread history read must not restart that timer. `context` contains `used` and `max` token counts last reported by the backend. A missing report stays null. `quota` contains `remaining_pct` and nullable `resets_at` in Unix seconds. Clients compute days left from that absolute timestamp instead of requesting a new event every second. The core omits an expired weekly quota and does not copy a Codex account's quota into another backend's session.

## Transcript rows and cards

An `Entry` contains `id`, `kind`, Markdown/source `text`, `done`, nullable `tool` and nullable `client_message_id`. Kinds are `user`, `reasoning`, `commentary`, `answer`, `tool`, `notice`. Clients wrap and style source text for their own width. No row carries a terminal dimension, vector offset, color or key binding.

A tool row carries its compact facts in `tool`; its `text` is empty. The client reads the large body with `entry/output` when needed. Tool facts contain `kind` (`command`, `read`, `search`, `edit`, `other`), `label`, `label_source` (`literal`, `ai`), `status` (`running`, `ok`, `failed`, `interrupted`, `unknown`), nullable `summary`, nullable `added` and `removed` counts, and `has_output`. Unknown facts remain unknown. EP018 exposes current labels, paths, diffs, completion and failure facts. EP021 supplies richer tool classifications, success/interruption facts, summaries and diff counts from native data. Clients must not infer success from `done: true` or from a null count.

Output line kinds are `plain`, `addition`, `deletion`, `hunk`, `context`, `command`, `success`, `error`. EP018 classifies diffs and otherwise supplies plain lines. A client can style those kinds without parsing terminal escape codes. Opening output is local in the terminal; a remote client fetches it once and caches it until a later event changes the row. A facts event for a tool also invalidates its cached output, even if the compact fields compare equal.

Live approval cards are separate from stored transcript rows because the core already owns them separately. A snapshot includes the active card and queued cards in `approvals`. Each card contains `id`, `kind`, `title`, `details`, `choices`, optional `input`, `submitted`, `active` and `default_to_no`. A card id comes from the same opaque allocator as rows. The core exports only display facts and selected input descriptions, never the native routing payload. A client shows choices the card offers; it must not assume every backend offers session-wide approval.

A card's `input.kind` is `fields`, `form`, `browser` or `unsupported`. A fields input carries neutral `fields`: each has a card-local `key`, `title`, `description`, `secret`, `multiple`, `other`, `notes_only`, `required`, `schema` and `options`. Each option separates its typed `value` from its `label` and `description`. Field keys belong to an answer within this card; they cannot address a session or backend callback. A form input carries its JSON `schema`, a browser input its `url`, and an unsupported input a `reason`. The core uses the same native question readers as the terminal, so a phone does not interpret Codex or Claude question messages. A browser card offers `once` for explicit consent; it does not require an empty form answer. `default_to_no` tells clients to keep a negative default and require explicit consent instead of treating Return as an automatic allow. The terminal's unsent form answers, browser consent, focus and confirmation hints stay in ViewState and never enter a snapshot. An accepted submission sets `submitted`; backend withdrawal and successful answers both retire the card. A resolved-card event carries a nullable `decision`: a known accepted choice is explicit, while null means the card closed without a known user decision. Clients must not label a backend withdrawal as an approval. The core does not persist resolved cards as transcript history in EP018; a new snapshot contains only live cards.

## Events and snapshots

The `session/event` notification contains `session`, `seq` and `event`. The `event.kind` selects one of these records:

- `entry_added` carries a complete `entry` in chronological append order.
- `text_appended` carries `entry` (id) and only the new `text`. Text is appended exactly, including Unicode and newlines.
- `entry_updated` carries a complete replacement `entry` with the same id. This event handles edits that cannot be represented by a suffix. All fields replace old values, including nulls and empty text.
- `entry_facts_changed` carries `entry` (id) and `facts` with `kind`, `done`, `tool`. It changes those facts without resending text. It also invalidates cached tool output.
- `approval_requested` carries a complete `approval` and nullable `before` card id. Remove that id from the local live-card list, then insert the card before `before` if that neighbor is present, or at the end otherwise. This preserves card order, focus moves and submitted state without exposing a storage offset.
- `approval_resolved` carries `approval` (id) and nullable `decision`. It removes that id from the live cards.
- `info_changed` carries complete `info`. It replaces all session header facts. Full objects avoid an ambiguous missing-versus-null patch convention.
- `reset` carries a complete `snapshot`. It replaces a client's stored view at that event's cursor. The snapshot cursor equals the reset event cursor.

A snapshot contains `info`, `entries`, `approvals`, `seq` and `has_older`. Entries form the newest page in chronological order; the default reset includes 100 rows. The core read API clamps page sizes to 1 through 1000. `has_older` tells a client to request earlier rows. EP018 pages the rows already held in memory; EP021 adds lazy loading of native history. The terminal still borrows the full contiguous core transcript and does not keep a second copy.

A client applies events only to the named session. The next cursor must equal the previous cursor plus one. It must not advance past an unknown event kind or a gap; it needs a replay or snapshot. It may ignore updates for older rows it has not loaded. Later history reads include current row facts and a cursor; the shared client in EP019 must merge a page at that cursor without overwriting newer events. An empty replay can confirm the current cursor. Replaying a reset replaces old pages and entry addresses as its snapshot directs.

The core coalesces writes before each `publish` call. New rows carry their final unpublished text once; several suffix writes on an existing row become one suffix. A later replacement supersedes earlier pending suffixes for that row. A completion or label change arrives even without new text. The event numbers order published facts, not every intermediate byte received from a backend. A caller must encode the batch before its next write; Rust enforces this through the batch's borrow of SessionState.

The core holds at most 256 distinct pending row edits and bounds each emitted record batch. Large bursts, history changes and overflow produce a reset instead of an unbounded log. A reset may carry a stable earliest changed id to the in-process terminal as a cache hint; that hint does not appear on the wire. The terminal reads the numbered record, resolves its id and invalidates the same suffix of its render cache. The core does not clone or serialize the whole transcript for each terminal redraw.

EP019 will retain owned events in a bounded replay ring and bound each client's outgoing queue. A slow client must not stall the session owner; the server replaces a dropped queue with a snapshot. The current borrowed batches deliberately contain no replay promise after a caller drops them. If a local caller drops a batch, the terminal detects its cursor gap and rebuilds from current core facts.

`sessions/changed` carries `kind: "updated"` with complete `info`, or `kind: "removed"` with `session`. The session manager in EP019 will emit those notifications.

## Compatibility and proof

Readers may ignore unknown object fields. Unknown method names, event kinds, enum variants, invalid ids and unsupported versions fail explicitly. A new optional object field can extend version 1 if its absence has the documented old meaning. Changing a required field, event meaning or enum requires a new protocol version or explicit capability negotiation; a client must never silently skip an unknown state change.

Run `cargo test -p carlos-proto` for envelope, version, id and cursor checks. Core event tests serialize real published events to JSON, apply them to a headless client's copy, and compare it with a snapshot after Unicode streaming, normalized reasoning, completion, labels, approvals and history replacement. Terminal checks drive the actual executable against deterministic peers. `.agents/reports/exec018-final-report.md` records the accepted source and measured results once this step is complete.
