## Why

The OpenCode plugin is already usable, but the current implementation, specifications, and user-facing documentation have drifted apart. We identified concrete fit-for-purpose gaps in auth UX alignment, session/capture correctness, stale OpenCode integration docs/specs, and missing practical guidance for human daily use.

## What Changes

- Align the OpenCode plugin specs and docs with the current supported plugin behavior, tool surface, and configuration model.
- Fix correctness gaps in the plugin lifecycle and capture path, including session startup, tool execution capture fidelity, and significance detection behavior.
- Clarify and harden the supported auth and runtime behavior for OpenCode plugin users, including documented expectations for device-flow sign-in and refresh/session reuse.
- Add practical end-user guidance for daily OpenCode usage, including how to retrieve context, capture memory, query knowledge, and promote stable insights.
- Evaluate and address fit-for-purpose gaps where the plugin currently relies on misleading or stale integration paths.

## Capabilities

### New Capabilities
- `opencode-plugin-usage`: Human-oriented OpenCode daily usage guidance for memory and knowledge workflows.

### Modified Capabilities
- `opencode-integration`: Update plugin installation, tool surface, lifecycle, capture, and documentation requirements to match the current supported implementation.
- `opencode-plugin-auth`: Clarify supported plugin auth UX, refresh/session reuse expectations, and fit-for-purpose behavior for interactive OpenCode use.
- `local-memory-store`: Align local-first plugin expectations with the actual OpenCode plugin behavior and offline/shared-layer routing model.

## Impact

- Affected code: `packages/opencode-plugin/src/**`, plugin hooks/tools/local store, related website/docs integration guides.
- Affected specs: `openspec/specs/opencode-integration/spec.md`, `openspec/specs/opencode-plugin-auth/spec.md`, `openspec/specs/local-memory-store/spec.md`, plus a new usage-oriented capability.
- Affected user workflows: OpenCode plugin installation, interactive sign-in, automatic context injection, memory capture, knowledge proposal, and session lifecycle behavior.
