# Design: LLM-backend adapter

## D1. Adapter contract

Each `e2e/llm/<backend>.sh` script implements **exactly four subcommands**:

```
provision   bring the backend up; idempotent; safe to call repeatedly
env         print KEY=VAL lines (one per line, no quotes) to stdout for sourcing
health      exit 0 iff backend is reachable AND a sample completion request succeeds
cleanup     tear down resources created by `provision`; idempotent
```

The `env` subcommand outputs **at minimum**:

```
AETERNA_LLM_PROVIDER=openai            # or google, bedrock
AETERNA_OPENAI_MODEL=<model-id>
AETERNA_OPENAI_BASE_URL=<url>          # consumed by Phase 2 Rust change
OPENAI_API_KEY=<token-or-placeholder>
```

Backends MAY emit additional vars (e.g. `OLLAMA_HOST`) that the test harness
ignores but operators may use for debugging.

Unknown subcommands MUST exit 64 (EX_USAGE) with a one-line usage message on
stderr. This is enforced by a contract test in Phase 1 task 4.

## D2. Backend matrix

### `ollama` — default for Tier 0 CI

- Spawned via GHA `services:` container (`ollama/ollama:latest`)
- Port 11434, OpenAI-compat endpoint at `/v1`
- Default model: `qwen2.5:0.5b` (~400MB, runs on 2-vCPU GHA-hosted runner)
- Override via `AETERNA_E2E_OLLAMA_MODEL` for local dev
- Provision pulls the model and waits for `/api/tags` to list it
- `env` outputs `OPENAI_API_KEY=ollama` (Ollama ignores the value but the
  OpenAI client refuses an empty key)

### `recorded` — fast-path replay

- Tiny Python HTTP server (`e2e/tools/mock-llm-server.py`, stdlib only)
- Listens on `127.0.0.1:$PORT` (port chosen by `provision` and pinned in env)
- Two modes selected by `AETERNA_E2E_LLM_RECORDED_MODE`:
  - `replay` (default): reads `e2e/fixtures/llm/<sha256>.json` keyed by a
    canonical hash of the request body. Missing fixture → 503 with a clear
    "missing fixture; re-record with mode=record" body, fail-loud.
  - `record`: forwards the request to a configured upstream
    (`AETERNA_E2E_LLM_RECORD_UPSTREAM=live-openai|live-anthropic|github-models`),
    saves the response under the request hash, returns it to the caller.
- Hash canonicalisation: sorted keys, normalised whitespace, fields
  `[temperature, top_p, seed]` rounded to fixed precision.
- Server PID written to `.e2e/llm-recorded.pid`; `cleanup` kills it.

### `github-models` — Tier 1 CI (real models, free for OSS)

- Endpoint: `https://models.github.ai/inference`
- Auth: `${GITHUB_TOKEN}` (default in GHA, also accepts a PAT for local dev)
- Default model: `openai/gpt-4o-mini` (cheap on the GitHub Models rate-limit
  budget; can be overridden via `AETERNA_E2E_LLM_MODEL`)
- Workflow guard: `if: github.event_name != 'pull_request'
  || github.event.pull_request.head.repo.full_name == github.repository`
  — fork PRs do not trigger this tier (use Tier 0 instead)
- `provision` is a no-op (no local resources); `health` does a 1-token call
  to confirm the token works

### `live-openai`, `live-anthropic` — local dev only

- `provision` checks the relevant env var is set; fails loud if not
- `env` outputs straight passthrough
- `cleanup` is a no-op
- These adapters are **never selected automatically** — the runner only
  uses them when `AETERNA_E2E_LLM_BACKEND` is set explicitly

## D3. Tier strategy

| Tier | Trigger                                  | Backend default        | Cost |
|------|------------------------------------------|------------------------|------|
| 0    | every push, every PR (incl. forks)       | `ollama` or `recorded` | $0   |
| 1    | push to `main`, scheduled nightly        | `github-models`        | $0   |
| 2    | manual `workflow_dispatch` only          | `live-openai`          | $$   |

Tier 0 default is `ollama` for new tests (real-but-dumb inference) and
`recorded` for legacy/long-tail tests where fixture replay is faster and
more deterministic.

For a PR labeled `e2e:fast`, the workflow uses `recorded` only, skipping
the ~90s ollama warm-up. For `e2e:full`, both Tier 0 and Tier 1 jobs run.

## D4. Fixture management for `recorded`

- Fixtures live at `e2e/fixtures/llm/<sha256-12>.json` (12-char prefix of
  the request hash, plus a sidecar `.meta.json` describing the original
  test name and upstream provider used at record time).
- Recording is triggered manually by a maintainer:
  ```
  AETERNA_E2E_LLM_BACKEND=recorded \
  AETERNA_E2E_LLM_RECORDED_MODE=record \
  AETERNA_E2E_LLM_RECORD_UPSTREAM=live-openai \
  OPENAI_API_KEY=sk-... \
    ./e2e/run.sh
  ```
- Recorded fixtures are committed to the repo. They are intentionally not
  large (chat completions are kilobytes) so this is fine.
- A `make refresh-fixtures` shortcut is provided.
- A CI job validates that no test produces a fresh recording in CI (i.e.
  every `recorded`-mode test must hit a committed fixture). If it doesn't,
  the suite fails with "missing fixture for test X — record locally and
  commit".

## D5. Caching strategy (CI cost discipline)

- **Ollama image**: cached by Docker layer cache via
  `actions/cache@v4` on key `ollama-image-${{ hashFiles('e2e/llm/ollama.sh') }}`
- **Ollama model blob**: cached at `~/.ollama/models` with key
  `ollama-model-qwen2.5-0.5b-v1`. Cache miss only when the model list
  changes — a typical PR run reuses the cache and skips the ~60s pull.
- **GitHub Models**: nothing to cache; rate-limited per user/org.
- **recorded fixtures**: in-repo, no caching needed.

Expected cold cache time-to-ready:
  - ollama: ~90s (image pull + model pull + warmup)
Expected warm cache:
  - ollama: ~10s (container start + model already on disk)

## D6. Phase 2 Rust changes — `OpenAiLlmConfig.base_url`

The smallest possible change:

```rust
// memory/src/llm/factory.rs
pub struct OpenAiLlmConfig {
    pub model: String,
    pub api_key: String,
    pub base_url: Option<String>,   // NEW — None = api.openai.com
}

impl OpenAiLlmConfig {
    pub fn from_env() -> Result<Self, LlmFactoryError> {
        Ok(Self {
            model: std::env::var("AETERNA_OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".into()),
            api_key: std::env::var("OPENAI_API_KEY")
                .map_err(|_| LlmFactoryError::Configuration("OPENAI_API_KEY not set".into()))?,
            base_url: std::env::var("AETERNA_OPENAI_BASE_URL").ok(),
        })
    }
}
```

The field threads into the OpenAI client constructor (currently in
`memory/src/llm/openai.rs` based on adjacent files — verify during BUILD).
The underlying HTTP client (`async-openai`, `reqwest`-based) supports
base-URL override; we wire the `Option<String>` through.

Backwards compatible: when `AETERNA_OPENAI_BASE_URL` is unset, behaviour is
unchanged (real api.openai.com). Existing tests do not need updates.

Applies to both `memory/src/llm/factory.rs` and
`memory/src/embedding/factory.rs` (embedding side has the same hardcoded
limitation; we'll fix both for symmetry, even if the e2e suite only
immediately exercises LLM completions).

## D7. Acceptance criteria

- **AC1** — All five `e2e/llm/<backend>.sh` scripts implement the contract
  (provision/env/health/cleanup). Verified by `e2e/llm/contract-test.sh`.
- **AC2** — `ollama` adapter brings up Ollama + qwen2.5:0.5b on a fresh
  Ubuntu 24.04 GHA-hosted runner in <120s cold and <30s warm. Health check
  passes a real 1-token completion.
- **AC3** — `recorded` adapter replays a committed fixture and matches the
  exact response body byte-for-byte. Missing fixture fails fast with a
  clear error referencing the test name.
- **AC4** — `github-models` adapter authenticates with `GITHUB_TOKEN` and
  completes a 1-token call against `openai/gpt-4o-mini`.
- **AC5** — `live-openai` and `live-anthropic` adapters fail-loud when
  their respective API key env vars are unset; pass when set.
- **AC6** — Phase 2: `AETERNA_OPENAI_BASE_URL=http://localhost:11434/v1`
  routes a `cargo run --bin aeterna-cli` completion request to a local
  Ollama instance. Verified by an integration test.
- **AC7** — Phase 2: When `AETERNA_OPENAI_BASE_URL` is unset, all existing
  unit tests pass unchanged.
- **AC8** — Tier 1 (github-models) job is gated such that a PR from a
  fork does not run it (verified by inspecting the rendered workflow YAML).
- **AC9** — Tier 0 (ollama) job runs on every PR including forks, with no
  secrets configured at the org level.
- **AC10** — The full e2e suite, with `AETERNA_E2E_LLM_BACKEND=ollama`, runs
  end-to-end on a default GHA-hosted runner with no manual setup beyond
  cloning the repo.

## D8. Risks and mitigations

- **R1: Ollama model output is too dumb for some tests.** Mitigation: write
  e2e tests to assert on aeterna's plumbing (response shape, error mapping,
  tracing spans, secret resolution path), not on LLM response content. For
  tests that DO need specific output, use `recorded` mode.
- **R2: GitHub Models rate limits collapse under heavy nightly load.**
  Mitigation: nightly runs use a single workflow job with serial test
  execution against this tier; parallel test jobs use `ollama` instead.
- **R3: Microsoft sunsets GitHub Models / changes terms.** Mitigation:
  the adapter contract makes swapping to a different free provider a
  one-script-file change. No aeterna code depends on this provider.
- **R4: Recorded fixtures rot when prompts change.** Mitigation: every
  test using `recorded` mode includes the prompt as part of the request
  hash; prompt changes naturally invalidate the fixture and trigger a
  re-record. The CI "no fresh recordings" check catches this loudly.
- **R5: Phase 2 base_url change breaks existing OpenAI users.** Mitigation:
  `Option<String>` defaults to `None`; behaviour identical when env var
  unset. Covered by AC7.
