# EP021a command records

The wire command and the Core command now use the same SetContext, SetSettings
and SetToolLabels records from carlos-proto. The daemon keeps the public session
address outside each record, selects its owner, and passes that owner the record.
The client still allocates an operation only for an explicit write. Core still
captures its own permission flag and backend target before native work starts.

The v1 JSON fields remain flat. The record's Rust field name `request` never
appears in the wire envelope. A golden test passed before the change and after
it. It checks exact serialized bytes for all three commands, including the
existing false default when save_default is omitted. The native mailbox still
uses the same byte charge, and CommandInfo still assigns ordinary write space.

The following records and translators remain because their fields or meaning
differ:

- Wire task commands carry public window handles. Core task commands carry a
  WindowContext with the daemon's private reader flag. Sharing the wire record
  here would discard the socket's checked reader right.
- Wire activity commands address opaque handles. Core activity commands carry
  ActivityContext and the original reader flag, and Core window/activity opens
  capture owner state. Neither record can be supplied by a remote client.
- A reopen wire request names a public session. The owner adds its accepted
  generation during admission. Keep that generation private rather than add it
  to a shared caller payload.
- A terminal NativeIntent keeps a saved settings revision and edited choices
  before the user submits them. SetSettings adds an allocated operation and the
  explicit save-default choice. The draft and submitted command represent
  different stages and must not be the same record.
- Backend SettingsPatch holds confirmed or requested native model/effort
  values. Protocol SettingsChange also supports inherited choices and summary
  policy. The owner must continue checking and translating those meanings.
- The selected task reader captures window/revision/task for later named reads.
  The request tracker checks the original reply id, and the low-level decoder
  target carries the session. A reply itself has no window field. Keep the
  captured window for continuations and the decoder checks for original replies.
- The terminal's private stop intent freezes its original connection, selected
  window/task, run proof and explicit stop choice. Common task data and raw run
  replies already pass through carlos-client's validators; the terminal still
  owns whether to send its original stop. Moving that consent into a display
  cache would mix native writes with common reads.

Other small command families keep their explicit enum fields. This change
shares the three settings payloads; it does not add a public record for every
variant or a generic command builder. No transport registry or wire-version
change was added.
