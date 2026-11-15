# EP019b flat form compatibility

The old editor tests accept two flat choice schemas: an array whose all-string `items.enum` omits `items.type`, and a string whose titled constant choices use `anyOf`. The actual daemon consumer rejected both after the editor parsed their defaults. The Claude helper also rejected them before showing an editable form. The old fake validation peer had not exercised that helper check.

The core now checks a temporary copy in the native schema's spelling: it supplies the implied string item type and spells the single-choice constant list as `oneOf`. The core still validates answer values against the original schema. It preserves the original schema, labels, option order, defaults and card fingerprint. The same size, field-count, external-reference, unknown-key and constraint checks remain in place.

The helper accepts those same two choice spellings and checks submitted values against their original choices. It still rejects explicit wrong types, mixed enum types, duplicate choices, conflicting choice keywords, unknown keys and external references. The former helper test that rejected the omitted item type now has a named positive counterpart; its negative slot tests a mixed enum instead. The explicit wrong-type case remains negative.

## Before and after

- The original eight-default editor table failed for both backends on exactly these two schemas: `/tmp/carlos-callback-default-schema-negative-02.log`. The guarded native-envelope table also rejected them: `/tmp/carlos-callback-envelope-focused-02.log`.
- The new actual daemon workflow failed on the old frozen pair before any answer: `/tmp/carlos-form-compat-native-before.log`, artifacts `/tmp/carlos-ep019b/form-compat-before/`. The helper exported “array items must be strings”; the screen offered decline/cancel instead of the expected Tags editor. This is a separate failure from the core's strict shape check.
- The unchanged new workflow passes on the repaired pair, together with three existing typed-form, unsafe-integer and unsupported/decline/cancel workflows: four passes in 3.708 seconds, `/tmp/carlos-form-compat-native-after.log`, artifacts `/tmp/carlos-ep019b/form-compat-after/`.
- That workflow asserts checked defaults on screen, then the exact original native id `legacy-form-question` and complete reply `{"action":"accept","content":{"a_tags":["a","b"],"b_choice":"y"}}`. It asserts one callback response, one original prompt and omission of the unused optional field.

## Checks and binary identity

- Twelve core schema tests pass, including unchanged original facts and explicit malformed/unknown/ref/duplicate negatives: `/tmp/carlos-form-compat-core.log`.
- Four native-envelope, four neutral-editor and three actual Device lifetime tests pass in the combined source tree. Their separate test-move checkpoint retains those original assertions; they are not part of this runtime commit.
- All 72 helper tests pass: `/tmp/carlos-form-compat-helper-tests.log`; TypeScript check passes: `/tmp/carlos-form-compat-helper-check.log`.
- The Rust pair and helper were built explicitly: `/tmp/carlos-form-compat-{pair,helper}-build.log`. Cargo's debug build alone retained the earlier helper, so the proof uses the separately rebuilt and hashed helper.
- Frozen pair: `/tmp/carlos-ep019b/form-compat-pair/`. Both Rust executables report source `e15a44ce2f35579999d58aa11d8d0e03`. The identity file records 2,117 source paths, Carlos SHA-256 `31deaaea15c44f9fc328fbf6b69916c6f2dbbcb8b4551703fb3919832ab7f4dc`, Carlosd `f4944c042da399ceee95c022c8d81e398b3d8b984eef512f2d0025b11789b127` and rebuilt helper `1cd57663cd84fc1d9dd9af062ef99868377a5934056203d96a8140878f8acd6a`.

The pair records the coherent combined worktree, including the separate materialized-start repair and pending test moves. It is not an isolated accepted prefix or a default-launcher cutover. The final whole-EP gate remains with the parent worker. This repair adds no protocol method, raw native route, scheduler or second editor.

The final strict no-default, all-target workspace Clippy, formatting and Rust 400/60 checks pass in `/tmp/carlos-form-compat-clippy-final.log`, `/tmp/carlos-form-compat-fmt.log` and `/tmp/carlos-form-compat-size.log`. The normal commit hook runs all workspace Rust tests; its log is `/tmp/carlos-form-compat-commit.log`.
