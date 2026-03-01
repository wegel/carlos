# Carlos Agent Notes

## App-Server Schema Extraction

Use the Codex CLI generator to refresh the local protocol schema bundle:

```bash
mkdir -p docs/app-server-schema
codex app-server generate-json-schema --experimental --out docs/app-server-schema
```

Optional TypeScript bindings snapshot:

```bash
mkdir -p docs/app-server-ts
codex app-server generate-ts --experimental --out docs/app-server-ts
```

Key files to inspect after generation:

1. `docs/app-server-schema/codex_app_server_protocol.schemas.json` (full bundled schema)
2. `docs/app-server-schema/ServerNotification.json` (notification method union)
3. `docs/app-server-schema/v2/ThreadTokenUsageUpdatedNotification.json` (token usage shape)
4. `docs/app-server-schema/v2/ContextCompactedNotification.json` (legacy compaction notification)

Current context indicator in `carlos` depends on `thread/tokenUsage/updated` payload fields:

1. `params.tokenUsage.modelContextWindow` (max context window)
2. `params.tokenUsage.total.totalTokens` (used tokens)
