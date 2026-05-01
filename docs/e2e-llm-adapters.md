# E2E LLM adapters

The e2e conformance suite uses a small shell adapter contract so the test
runner can switch between free local inference, recorded fixtures, and
real hosted providers without changing the Rust process configuration.

## Contract

Every adapter in `/e2e/llm` implements the same four subcommands:

```bash
e2e/llm/<backend>.sh provision
e2e/llm/<backend>.sh env
e2e/llm/<backend>.sh health
e2e/llm/<backend>.sh cleanup
```

- `provision` brings the backend up and is safe to call repeatedly
- `env` prints `KEY=VALUE` lines for sourcing into the test process
- `health` exits 0 only when the backend is actually usable
- `cleanup` tears down any resources created by `provision`

Unknown subcommands exit with code `64`.

## Supported backends

| Backend | Tier | Cost | Secrets | Best for |
|---|---|---|---|---|
| `ollama` | 0 | $0 | none | every PR, including forks |
| `recorded` | 0 | $0 | none | deterministic fixture replay and fast-path CI |
| `github-models` | 1 | $0 | `GITHUB_TOKEN` | pushes to main and nightly smoke coverage |
| `live-openai` | 2 | paid | `OPENAI_API_KEY` | local development and manual dispatch |
| `live-anthropic` | 2 | paid | `ANTHROPIC_API_KEY` | forward-compatible local wiring only |

## Which backend should I use?

- Use **`ollama`** when you want a real OpenAI-compatible backend with no
  secrets.
- Use **`recorded`** when the test needs deterministic output or you want the
  fastest possible run.
- Use **`github-models`** for hosted-provider smoke coverage on trusted
  branches.
- Use **`live-openai`** only when you explicitly want to spend API credits.
- Use **`live-anthropic`** only for adapter-level checks; the runtime factory
  does not yet route anthropic traffic.

## Recorded fixtures

`recorded` uses `e2e/tools/mock-llm-server.py` to replay committed fixtures from
`e2e/fixtures/llm/`. Missing fixtures fail loudly with the request hash so a
maintainer can re-record them locally.

Example record flow:

```bash
AETERNA_E2E_LLM_BACKEND=recorded \
AETERNA_E2E_LLM_RECORDED_MODE=record \
AETERNA_E2E_LLM_RECORD_UPSTREAM=live-openai \
OPENAI_API_KEY=sk-... \
  ./e2e/run-e2e.sh
```

## Runtime configuration bridge

All OpenAI-compatible adapters emit:

- `AETERNA_LLM_PROVIDER=openai`
- `AETERNA_OPENAI_MODEL=...`
- `AETERNA_OPENAI_BASE_URL=...`
- `OPENAI_API_KEY=...`

The Rust runtime reads `AETERNA_OPENAI_BASE_URL` for both chat completions and
embeddings, which makes the same process work against Ollama, GitHub Models,
and the recorded replay server.

See also:

- `e2e/llm/README.md`
- `docs/guides/provider-adapters.md`
