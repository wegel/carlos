# EP021b native contracts

## Grok and ACP tool facts

On2026-11-16, /usr/local/bin/grok --version reports grok1.0.77 (50ca579fa996) [alpha]. The existing process.rs sends protocolVersion1. Read the official [ACP v1 tool-call contract](https://github.com/agentclientprotocol/agent-client-protocol/blob/main/docs/protocol/v1/tool-calls.mdx) and [v1 schema](https://github.com/agentclientprotocol/agent-client-protocol/blob/main/schema/v1/schema.json), not the newer v2 rules.

ACP v1 defines native read/search/edit/delete/move/execute categories. A tool update can omit unchanged fields; completed means success and failed means an error. Preserve those facts in GrokTranslationState and the existing shared Tool fields. The adapter's internal item/toolFacts/updated message changes an existing entry's facts without replacing its source, exposing a hidden body or starting label inference. It adds no daemon/client command or wire family. Generic Codex completion without an exit result still meansUnknown.

The real process fixture sends opaque Unicode titles, a successful read, successful command and failed search. The accepted EP021a daemon reportsOther/Unknown and fails in0.393s for that exact mismatch, /tmp/carlos-ep021b/grok-tool-facts-baseline.log. The new adapter passes the same real process/socket test. A first fixture import depended on child-only environment and was fixed; a later wait assumed a visible search body, but existing Grok code hides search and read bodies. Preserve that behavior. Those setup traces remain separate from the intended baseline failure.

## Grok permission choices, before implementation

The same official v1 contract defines session/request_permission with the original JSON-RPC id, native sessionId, toolCall and offered options. A selected answer returns the original optionId; a cancelled answer grants nothing. The agent owns the scope and lifetime of remembered choices, so allow_always does not inherently mean “this session.” Do not silently map it to a session-only promise.

The existing reader auto-allows numeric-id permission requests and fails to recognize string-id requests. create_session/load_session force _meta.yoloMode:true. The official [Grok agent guide](https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/docs/user-guide/15-agent-mode.md) documents that flag as always-approve and describes normal interactive permission prompts. That guide is current upstream, not a claim that its source revision equals the installed alpha binary; the public commit URL for50ca579fa996 returned404. Keep the installed version and actual fixture/contract evidence separate.

Before changing this path, prove the offered-choice and startup callback behavior through the actual process fixture. Reuse shared approval controls and original writer guards. Do not manufacture an allow option, alter native files or infer a remembered permission's scope from its enum alone. Approval implementation and native history contract checks are still pending.
