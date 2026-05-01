# Recorded LLM fixtures

Files in this directory are content-addressed by the canonical hash of an
LLM request body. Used by `e2e/llm/recorded.sh`.

## File layout

```
<sha256-prefix-12>.json        # the upstream response body, verbatim
<sha256-prefix-12>.meta.json   # { test_name, upstream, recorded_at, model }
```

## Refresh

```
AETERNA_E2E_LLM_BACKEND=recorded \
AETERNA_E2E_LLM_RECORDED_MODE=record \
AETERNA_E2E_LLM_RECORD_UPSTREAM=live-openai \
OPENAI_API_KEY=sk-... \
  ./e2e/run.sh
```

Fixtures are intentionally committed to the repo — chat completions are
kilobytes and reproducibility beats repo-size purity.
