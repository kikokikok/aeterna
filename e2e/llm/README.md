# LLM-backend adapters for the e2e conformance suite

Each adapter exposes the same shell contract:

```
e2e/llm/<backend>.sh provision   # bring up; idempotent
e2e/llm/<backend>.sh env         # print KEY=VAL lines for sourcing
e2e/llm/<backend>.sh health      # exit 0 iff ready
e2e/llm/<backend>.sh cleanup     # tear down; idempotent
```

Unknown subcommands exit 64 (EX_USAGE).

## Backends

| Backend | Tier | Cost | Secrets | Use |
|---|---|---|---|---|
| `ollama` | 0 | $0 | none | every PR (incl. forks) |
| `recorded` | 0 | $0 | none | fast-path replay |
| `github-models` | 1 | $0 | `GITHUB_TOKEN` | nightly + main, no forks |
| `live-openai` | 2 | $$ | `OPENAI_API_KEY` | local dev / dispatch only |
| `live-anthropic` | 2 | $$ | `ANTHROPIC_API_KEY` | local dev (runtime support pending) |

See `openspec/changes/add-llm-backend-adapter/design.md` for full design.
