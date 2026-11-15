# Escape keeps a waiting blocking question visible

The old Codex question test required Escape to leave a nonblocking draft and show an already-waiting blocking request. The actual daemon terminal hid both cards instead. Both native requests remained pending, but the terminal showed its main composer and only an F4 badge.

`test_daemon_escape_from_nonblocking_question_promotes_the_waiting_blocking_card` reproduces that failure with numeric native ID `9` and string native ID `"9"`. It waits for the terminal's `[2 waiting]` frame while the original nonblocking answer remains visible, then presses Escape. The frozen old executable fails at the blocking card's screen assertion in 8.707 seconds. Log: `/tmp/carlos-question-escape-before.log`; artifacts: `/tmp/carlos-ep019b/question-escape-before`.

The fix calls the existing `sync_questions(false)` immediately after hiding the nonblocking card. The existing local chooser selects an eligible blocking card. Core's active card, both native requests and both private drafts stay unchanged. No new focus state or native command exists.

The actual Device test `escape_promotes_an_existing_blocking_card_without_answering_or_changing_shared_focus` checks both branches: another nonblocking card stays hidden; a blocking card becomes visible. The original answer, main composer and both accepted shared cards survive. No operation is allocated. Log: `/tmp/carlos-question-escape-device-02.log`.

The actual daemon terminal case passes in the three-case run recorded in `/tmp/carlos-question-goal-private-native-02.log` (all three pass in 1.963 seconds). It asserts no answer/chat requests, retires only string `"9"` through the native peer, then reopens the original numeric card with its private answer intact. The matching pair is frozen at `/tmp/carlos-ep019b/question-goal-private-pair/identity.json`, shared Rust source marker `7e15d1530e25fa10e004a2148f71d5e7`.

The current default workspace passes 1,627 tests with ten ignored; strict all-target Clippy passes with and without default features. Format and whole-tree style checks pass. Logs: `/tmp/carlos-question-goal-private-{workspace,clippy,nodefault-clippy,fmt,size}-02.log`. These checks include the sibling's committed native test ports and the goal-private test cases. Root owns the final joined terminal/native/release gate and embedded-path deletion. No reviewer, install or EP approval ran.
