# Tasks: LLM-backend adapter

## 0. Decisions to confirm before BUILD

- [ ] 0.1 Adapter contract shape — `provision|env|health|cleanup` (per design.md §D1). Locks the runner contract from #169 task 22.
- [ ] 0.2 Five backends in v1 — ollama, recorded, github-models, live-openai, live-anthropic (per §D2). Anthropic runtime support is out-of-scope for this PR.
- [ ] 0.3 Default model `qwen2.5:0.5b` for ollama (per §D2). Picks the smallest viable model for free-tier GHA runners.
- [ ] 0.4 Fixture format — content-addressed sha256-12 prefix, sidecar .meta.json, committed to repo (per §D4).
- [ ] 0.5 Tier strategy — Tier 0 default `ollama` for new tests / `recorded` for legacy; Tier 1 `github-models` on main+nightly only; Tier 2 manual (per §D3).
- [ ] 0.6 Phase 2 in-scope — the Rust `base_url` change is on the critical path and ships in this same openspec / PR (per proposal Phases section). Not a follow-up.

## Phase 1 — e2e adapter infrastructure

### 1. Scaffolding

- [ ] 1.1 Create `e2e/llm/` directory with a `README.md` documenting the contract and listing supported backends.
- [ ] 1.2 Create `e2e/llm/_lib.sh` with shared helpers: `log()`, `die()`, `require_cmd()`, `wait_for_url()`, `random_port()`.
- [ ] 1.3 Create `e2e/fixtures/llm/` directory with a `.gitkeep` and a `README.md` explaining the fixture format and the `make refresh-fixtures` workflow.

### 2. `e2e/llm/ollama.sh`

- [ ] 2.1 `provision`: detect whether running inside a workflow with the `services.ollama` container already up (port 11434 reachable); if so, no-op. Otherwise `docker run -d ... ollama/ollama:latest`. Pull `${AETERNA_E2E_OLLAMA_MODEL:-qwen2.5:0.5b}` via `POST /api/pull` and block until present in `GET /api/tags`.
- [ ] 2.2 `env`: emit `AETERNA_LLM_PROVIDER=openai`, `AETERNA_OPENAI_MODEL=${MODEL}`, `AETERNA_OPENAI_BASE_URL=http://localhost:11434/v1`, `OPENAI_API_KEY=ollama`.
- [ ] 2.3 `health`: POST a 1-token completion to `${AETERNA_OPENAI_BASE_URL}/chat/completions`; exit 0 iff HTTP 200 and JSON has `.choices[0].message`.
- [ ] 2.4 `cleanup`: stop the container if we started it; leave a workflow-managed service container alone.

### 3. `e2e/llm/recorded.sh` and `e2e/tools/mock-llm-server.py`

- [ ] 3.1 Write `mock-llm-server.py`: stdlib-only `http.server.BaseHTTPRequestHandler`. POST `/v1/chat/completions` and `/v1/embeddings`. Compute canonical request hash. In `replay`, look up `e2e/fixtures/llm/<hash>.json` → 200 with body, or 503 with diagnostic. In `record`, forward to upstream and persist.
- [ ] 3.2 Canonical hash: `sha256(json.dumps(canonical(req), sort_keys=True, separators=(",",":")))[:32]`. Canonical fn drops `[stream, user]` and rounds `[temperature, top_p]` to 2 decimal places.
- [ ] 3.3 `recorded.sh provision`: pick a free port, start the server with `--port $PORT --mode ${AETERNA_E2E_LLM_RECORDED_MODE:-replay}`, write PID to `.e2e/llm-recorded.pid`, wait for `/healthz`.
- [ ] 3.4 `recorded.sh env`: emit `AETERNA_OPENAI_BASE_URL=http://127.0.0.1:$PORT/v1`, `OPENAI_API_KEY=recorded`.
- [ ] 3.5 `recorded.sh health`: GET `/healthz`.
- [ ] 3.6 `recorded.sh cleanup`: kill the PID; rm pid file.
- [ ] 3.7 Commit one example fixture so the contract test in task 4 has something to replay.

### 4. `e2e/llm/github-models.sh`

- [ ] 4.1 `provision`: assert `GITHUB_TOKEN` is set; no-op otherwise.
- [ ] 4.2 `env`: emit `AETERNA_LLM_PROVIDER=openai`, `AETERNA_OPENAI_BASE_URL=https://models.github.ai/inference`, `AETERNA_OPENAI_MODEL=${AETERNA_E2E_LLM_MODEL:-openai/gpt-4o-mini}`, `OPENAI_API_KEY=$GITHUB_TOKEN`.
- [ ] 4.3 `health`: 1-token chat completion against `${AETERNA_OPENAI_BASE_URL}/chat/completions`.
- [ ] 4.4 `cleanup`: no-op.

### 5. `e2e/llm/live-openai.sh` and `e2e/llm/live-anthropic.sh`

- [ ] 5.1 `live-openai.sh provision`: assert `OPENAI_API_KEY` set; print confirmation only.
- [ ] 5.2 `live-openai.sh env`: emit `AETERNA_LLM_PROVIDER=openai`, `AETERNA_OPENAI_BASE_URL=https://api.openai.com/v1`, `AETERNA_OPENAI_MODEL=${AETERNA_E2E_LLM_MODEL:-gpt-4o-mini}`, `OPENAI_API_KEY=$OPENAI_API_KEY`.
- [ ] 5.3 `live-openai.sh health` + `cleanup` analogous to live-anthropic.
- [ ] 5.4 `live-anthropic.sh`: same shape; emits `AETERNA_LLM_PROVIDER=anthropic` (will not currently route in aeterna runtime — documented limitation per proposal Out-of-Scope).

### 6. Contract test

- [ ] 6.1 `e2e/llm/contract-test.sh`: for each backend script, asserts that:
  - `<script> bogus-subcommand` exits 64
  - `<script> env` produces output that, when sourced, sets at minimum `AETERNA_LLM_PROVIDER`, `AETERNA_OPENAI_BASE_URL`, `OPENAI_API_KEY`, `AETERNA_OPENAI_MODEL`
  - `<script>` is executable and shellcheck-clean
- [ ] 6.2 The contract test runs in CI as part of every PR.

### 7. GHA workflow snippets

- [ ] 7.1 `.github/workflows/e2e-tier0.yaml`: runs on every push and PR. Uses `services.ollama` container + `actions/cache` for the model blob. Calls `e2e/run.sh --llm-backend ollama`.
- [ ] 7.2 `.github/workflows/e2e-tier1.yaml`: triggers on `push` to `main` and on schedule (cron nightly). Uses github-models. Includes the fork-PR guard from §D2.
- [ ] 7.3 `.github/workflows/e2e-tier2.yaml`: `workflow_dispatch` only, takes `backend` input ∈ {live-openai, live-anthropic}. Documented in CONTRIBUTING.
- [ ] 7.4 Each workflow uploads test logs and (on failure) the full HTTP traffic captured by the mock server (when applicable).

## Phase 2 — Rust runtime: `AETERNA_OPENAI_BASE_URL`

### 8. `OpenAiLlmConfig.base_url`

- [ ] 8.1 In `memory/src/llm/factory.rs`, add `pub base_url: Option<String>` to `OpenAiLlmConfig`. Update `from_env()` to read `AETERNA_OPENAI_BASE_URL`.
- [ ] 8.2 Same change in `memory/src/embedding/factory.rs` for `OpenAiEmbeddingConfig` (or whatever the symmetric struct is named — verify during BUILD).
- [ ] 8.3 Thread `base_url` into the OpenAI client constructor in `memory/src/llm/openai.rs` (and the embedding equivalent). Use `async_openai::config::OpenAIConfig::with_api_base(...)` if `async-openai` is the underlying crate; otherwise pass to the reqwest client builder.
- [ ] 8.4 Update `OpenAiLlmConfig::default()` and any `..Default::default()` test-call-sites to set `base_url: None`.

### 9. Tests

- [ ] 9.1 Add a unit test in `memory/src/llm/factory.rs`:
  - `with_unset_base_url_keeps_default()` — `from_env()` returns `None` when env var unset; behaviour unchanged.
  - `with_set_base_url_propagates()` — `from_env()` returns `Some(...)` when set.
- [ ] 9.2 Add an integration test (gated by `#[cfg(feature = "e2e")]` or `IGNORE` by default) that constructs a client with a custom `base_url` and asserts the HTTP request goes there (using `wiremock` or similar test double).
- [ ] 9.3 Run full `cargo test --workspace` to confirm AC7 (no behaviour change when env var unset).

### 10. Documentation

- [ ] 10.1 Update `README.md` (or `docs/configuration.md` if present) with a row for `AETERNA_OPENAI_BASE_URL`.
- [ ] 10.2 Add a `docs/e2e-llm-adapters.md` page describing the contract, listing the five backends, and explaining when to use each.
- [ ] 10.3 Cross-link from PR #169's `e2e/README.md` (added there in #169 BUILD) to `docs/e2e-llm-adapters.md`.

## 11. Verification (Stage 6)

- [ ] 11.1 `bash -n` clean on all five adapter scripts and `contract-test.sh`.
- [ ] 11.2 `shellcheck` clean on same.
- [ ] 11.3 `cargo check --workspace`, `cargo clippy --workspace`, `cargo test --workspace` all pass.
- [ ] 11.4 Local smoke test: `e2e/llm/ollama.sh provision && e2e/llm/ollama.sh health && e2e/llm/ollama.sh cleanup` against a local Docker daemon completes successfully.
- [ ] 11.5 GHA dry-run via `act` or by pushing to a draft PR — confirm Tier 0 workflow comes up green on a fresh runner.
- [ ] 11.6 Confirm no regression on PR #170 (helm template still clean) and no behaviour change on existing aeterna unit tests (AC7).
