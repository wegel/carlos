# Route core reports through the daemon

EP019a adds the basic daemon routes described here. EP019b adds the remaining terminal families and deletes the old report enums and terminal bridge. This map describes each current report field; it does not claim all target routes already exist. The session owner must consume every report, including nested reports and follow-up actions, before acknowledging a native frame.

## Three destinations

A shared fact describes a session, entry or live card. The owner records it in SessionState and publishes it in the numbered session stream. A new snapshot must recover the same fact without replaying a synchronous return value.

A private result describes a particular accepted client command. The owner ties it to ClientOperation and retains it in a bounded result store that survives socket disconnect. Only that client can query or receive it. Native ids never serve as client addresses.

A device hint changes an editor, focus, selection or display cache. The daemon deletes that hint from its path. The client derives its own behavior from shared facts and its own command results. The terminal may consume legacy hints only until EP019b cuts over.

## BackendUpdate and shared wrappers

- BackendUpdate.change selects the current report family; it is never a public wire envelope. Consume each variant below. BackendUpdate.nested contains an already-applied history report and must be consumed recursively, including its action, before the next native frame.
- BackendChange.Items carries ItemChanges. reset_navigation is a device hint. history_replaced is a device hint over the shared reset/snapshot already recorded by core. Neither flag replaces the numbered transcript stream.
- BackendChange.Installed carries NativeInstalled. facts.history follows ItemChanges; facts.goal follows GoalUpdate below. editor is an unsent device draft and must be absent from the daemon path. Core history adoption and native connection facts must already be recorded before publishing a reset.
- BackendChange.Common, Claude, Codex and Closed select the maps below. None contributes no extra route.
- ThreadChanged.codex_reset is a device reset hint over shared connection/history changes. callbacks contains CallbackRetired values mapped below. The owner never sends a native callback key to a client.

## ClaudeUpdate and ClaudeEffect

ClaudeUpdate.items follows ItemChanges. effects is an ordered list of the variants below. action requests backend work, not UI work; handle it through the owner/worker rules below. history_frame is internal decoded native input consumed by finish_claude_update, never client data. Its resulting nested report must still be handled.

- Callback carries ClaudeCallback. Ignored adds nothing. Unsupported already records a shared notice. Accepted.id is internal native routing; card and retired become shared live-card changes. description becomes the card's neutral input description, never a terminal editor object.
- Cancelled.id is internal routing; Cancelled.card becomes shared card retirement. The client drops only hints attached to the retired card.
- Browser carries an internal native callback id. Browser URL/consent facts belong in the live card; each client opens its own browser after an explicit choice. Never issue a device browser command from the daemon.
- Thread follows ThreadChanged. HistoryReset follows the numbered reset. Cleared follows ApprovalCleared. Cards contains shared retired ids, already recorded by the card owner. CodexRetired follows CallbackRetired.
- CodexUnfocus, SwitchControls and ResetControls are device hints and have no daemon route.
- Submission carries SubmissionOutcome, a private result tied to its originating operation. Automatic work without a human owner has no private recipient. Do not broadcast recovered text into another client's composer.
- SessionClosed is a shared connection/availability fact and must stop accepting work against that native generation. Keep its durable session description available for explicit recovery.

## TurnUpdate

TurnUpdate.changes follows ClaudeUpdate. hint is a private diagnostic for the current command; remove key instructions and keep the client's recovery choice local. clear_selection and rewound are device hints over the accepted command/reset. submitted and name_saved are legacy text-match acknowledgements, replaced by operation-owned acceptance. Do not clear an editor because it happens to contain equal text. The accepted user entry carries ClientOperation; the accepted session name appears in shared SessionInfo. EP019a handles sends; EP019b adds rename/rewind consumers and removes these legacy fields.

## CommonUpdate, approvals and closure

CommonUpdate.items follows ItemChanges. thread follows ThreadChanged. approval.current and approval.retired are shared card changes; approval.clear_extra_lines is a device hint. cleared follows ApprovalCleared. clear_quota_countdown is a device hint: the client computes a countdown from shared absolute quota timestamps. action uses the owner/worker route.

ApprovalCleared.Codex follows CallbackRetired. Other.editor is an internal native id used by the old terminal editor; delete it from the wire. Other.cards contains shared retirements. CallbackRetired.key remains internal; active is a device focus hint, and cards contains shared retirements. A submitted card remains claimed after an uncertain native answer; disconnect or timeout cannot let another client answer it again.

CodexClosed.transcript follows ItemChanges; shared notices, connection phase and retired cards must already be recorded by the owner. retired follows shared card retirement. recovery is a private result for the queued mutation that supplied the text, never unowned session recovery. settings follows SettingsUpdate. hint becomes a shared backend diagnostic plus private command failure where applicable, with device key names removed. EP019a handles basic closure and pending sends; EP019b must correlate native queue/settings recovery before exposing those families.

## CallbackUpdate and remaining native families

CallbackUpdate.action uses the owner/worker route. effect selects one of the following. EP019a implements live cards, checks, answers and reply failures; EP019b implements goal, queue, panel and settings families before terminal cutover.

- None adds nothing. ReplyFailed.id is internal routing; error becomes a private result for the command that claimed this card. The shared card also records its submitted/uncertain status.
- Accepted.key is internal routing. editor becomes the neutral live-card input description, while card and blocking become shared facts. active may preserve shared card ordering but cannot choose a device's focused editor.
- Retired follows CallbackRetired. Checked.id stays internal. Checked.update.clear_notice is a device hint; unsupported changes the shared card description/capability; error is private to the submitted answer. reply is prepared immutable backend work for the matching claimed card, not a direct blocking send.
- Items follows ItemChanges. Goal carries GoalUpdate: clear_error and clear_editor are device hints over typed goal state and an operation-owned result. EP019b publishes the actual goal facts retained by SessionState.
- Queue carries QueueUpdate. show, select_first, clear_edit and clear_error are device hints. recovery is private to the queue mutation that supplied it. EP019b publishes typed queue facts with stable queue ids and explicit read targets.
- Panel carries PanelUpdate. selected, detail and reset_scroll are device hints. EP019b replaces the global selected target with each connection's explicit read interests and publishes typed task facts.
- Controls.interrupt contains internal callback keys and a message. Retire the matching shared cards and return a private result to the interrupt owner; never expose keys. Controls.settings and Controls.queue follow their respective maps.

SettingsUpdate.accepted acknowledges one settings command privately; actual accepted values/capabilities become shared facts in EP019b. Its effects remain ordered. ClearRejected is a device hint. Rejected.desired/message belongs to the submitting operation's private result; do not broadcast a draft. Persist contains host defaults to write on a worker only after acceptance. Notice is a backend diagnostic with key names removed; publish a shared notice only when it describes a shared backend condition, otherwise return it privately.

## Backend actions and timers

ServerRequestAction is internal work, never a generic backend passthrough on the client protocol. The owner prepares immutable data and captures the native target; workers execute waits and disk writes, then return checked completions. Keep existing request tokens and deadlines for reads already asynchronous. Do not put every read behind the ordinary mutation fence.

ReplyNative.card_id/result answers only the matching atomically claimed live card. ReplyError.request_id/code/message rejects an internal native request. ValidateCodexForm.id/schema/answer/revision validates a bounded candidate for the claimed card and returns through the existing checked native event. These routes belong to EP019a's approval work. The owner must not let a later callback answer bypass validation.

ClaudeControl.method/params, ApplyClaudeSettings and RefreshContext route startup/history/context reads needed by EP019a through captured transport work. EP019b adds the remaining task/settings controls with typed public commands. CodexQueuePage.token/cursor keeps the existing bounded asynchronous native queue paging tokens in EP019b. Ralph timers and prompt-file reads stay owner/worker work in EP019a; the daemon must not add another automatic driver beside a native goal.

## Proof and deletion gate

Tests must recover shared state from a fresh snapshot, retain a private send/answer result across disconnect, isolate two clients' editors and read targets, reject two answers to one card, and reject stale worker completions. An exhaustively matched report variant must not disappear through a catch-all branch. EP019b deletes the temporary report bridge, old enums and embedded backend/session path in the explicit deletion commit specified in docs/terminal-client-migration.md.
