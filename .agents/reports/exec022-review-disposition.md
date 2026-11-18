# EP022 end-review decisions

Fable 5.2 medium gave PASS WITH ISSUES on the frozen candidate. It found one
major defect. Root independently found the same defect while looking at how the
phone consumes a private submission result. The full static review and its
scope limits are in `exec022-fable-final-review.md`.

- The phone releases a textless rejected or uncertain result after its editor
  and visible error consume it. For an approval it clears that operation's
  button state so the current card can be answered again. Restore moves the
  exact retained text into an empty composer and releases the old record.
  Discard does the same cleanup after the user chooses to remove the retained
  copy. A failed fork keeps its separate text even if the edit sheet closes.
  The user can still copy, restore or discard it without affecting the main
  draft. When the user switches sessions, accepted sends no longer need a
  local echo for the departed session. Pending, queued and failed text remain
  attached to their original session and operation.
- The phone still rebuilds the visible list projection from the loaded rows.
  UIKit owns cell reuse and preserves the reader's place. A future phone
  performance pass should measure several thousand loaded entries during
  active streaming, then use the existing row change information to reduce
  whole-window work when that measurement warrants it.
- Share the platform-neutral approval field encoding and matching command
  guards when the web client needs them. The daemon already validates the
  submitted choices. The Rust bridge keeps the current iPhone call surface
  focused on the present screens. One shared encoder can serve the web, phone
  and terminal after the web flow exercises the required fields.
- The phone's Markdown structure lives in the shared client. The terminal has
  its own renderer. Reuse the shared structure for the web and measure any
  parser dependency cost before changing the daemon's features or the existing
  terminal renderer.
- The phone forces a reconnect after backgrounding. A half-open connection
  while the phone stays in the foreground can still appear connected until
  the next write or transport error. A later phone reliability check can
  reproduce that case and measure an explicit ping deadline.
- `PhoneHostModel` still coordinates more state than the other view models.
  Keep its readiness flags display-only. Rust checks readiness, identity and
  revision before it admits a command. Split private operation tracking from
  transcript caching if the next feature changes both independently.
