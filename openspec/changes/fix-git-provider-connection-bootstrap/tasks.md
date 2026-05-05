## 1. Stable shared connection IDs

- [x] 1.1 Add optional explicit `id` support to shared Git provider connection creation and preserve UUID generation when omitted.
- [x] 1.2 Validate shared connection IDs and reject duplicates in both in-memory and Redis-backed registries.
- [x] 1.3 Add API/runtime tests for explicit IDs, generated IDs, invalid IDs, and duplicate IDs.

## 2. Startup bootstrap seeding

- [x] 2.1 Add bootstrap seed-file loading for shared Git provider connections and reconcile allow-lists for matching existing records.
- [x] 2.2 Fail startup on immutable metadata drift for an already-seeded shared connection ID.
- [x] 2.3 Add bootstrap-focused tests covering seed create, allow-list reconciliation, and drift detection.

## 3. Chart wiring and docs

- [x] 3.1 Mount the chart-rendered shared connection seed file into the Aeterna deployment and expose its path through env.
- [x] 3.2 Update chart comments/docs so operators know shared Git provider connections are bootstrapped at server startup from `gitProviderConnections`.
