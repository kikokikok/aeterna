# Proposal: LLM-backend adapter for the e2e conformance suite

## Context

PR #169 (`redesign-e2e-conformance-suite`) introduces a multi-backend e2e suite
for `aeterna`. Its initial design implicitly assumed a real LLM provider would
be available in CI via `OPENAI_API_KEY` or similar. That assumption is wrong
for this project:

- **Public repository.** Real provider keys cannot be exposed to fork PRs
  without a `pull_request_target` security risk.
- **No funded subscription.** The maintainer has neither personal LLM API
  credits nor a budget line to spend on CI inference. Every `cargo test`
  must remain free.
- **Unbounded contributor cost.** A test matrix of N backends × M tenant
  flows × K runs/day × F forks rapidly becomes uncapped spend.

Without a fix, the e2e suite is either (a) gated on paid keys and effectively
never runs in public CI, or (b) gated on flaky free-tier providers that
violate ToS for automated testing and disappear without notice. Both kill the
value of the conformance suite.

Meanwhile, `OpenAiLlmConfig` in `memory/src/llm/factory.rs` hardcodes the
OpenAI endpoint — there is no `base_url` knob — so even if we wanted to
point aeterna at Ollama or GitHub Models today, it would not work end-to-end.

## Proposal

Introduce an **LLM-backend adapter contract** for the e2e suite, and add the
minimal Rust support needed to make the contract reachable from a running
`aeterna` process. Mirrors the secrets-backend pattern established by #169
and the OpenBao deployment-mode pattern in #170.

Five backends ship in v1:

| Backend         | Where it runs               | Cost  | Secrets required          | Default in            |
|-----------------|-----------------------------|-------|---------------------------|-----------------------|
| `ollama`        | GHA service container       | $0    | none                      | every PR (Tier 0)     |
| `recorded`      | Local Python replay server  | $0    | none                      | fast-path PR runs     |
| `github-models` | Microsoft inference (OSS)   | $0    | `GITHUB_TOKEN` (built-in) | nightly + main pushes |
| `live-openai`   | api.openai.com              | $$    | `OPENAI_API_KEY`          | local dev only        |
| `live-anthropic`| api.anthropic.com           | $$    | `ANTHROPIC_API_KEY`       | local dev only        |

All five expose the same shell contract:

```
e2e/llm/<backend>.sh provision   # bring up; idempotent
e2e/llm/<backend>.sh env         # print KEY=VAL lines for sourcing
e2e/llm/<backend>.sh health      # exit 0 iff ready
e2e/llm/<backend>.sh cleanup     # tear down
```

The runner from #169 sources `env` output into the aeterna process — it stays
backend-agnostic.

## Phases

**Phase 1 — e2e infrastructure (this change, first commit):**
  Adapter scripts, mock-LLM replay server, GHA workflow snippets, fork-PR
  safety, model and image caching strategy. Pure shell + Python; no Rust.

**Phase 2 — runtime base_url support (this change, second commit):**
  Add `base_url: Option<String>` to `OpenAiLlmConfig`, read from
  `AETERNA_OPENAI_BASE_URL`. Thread through the OpenAI client constructor
  in `memory/src/llm/openai.rs` (or wherever the client is built). Without
  Phase 2 the Ollama and GitHub Models adapters cannot reach aeterna's
  inference path. Phase 2 is on the critical path, not deferrable.

**Out of scope (separate future changes):**
  - Anthropic provider support in the runtime factory (currently absent;
    the CLI setup wizard collects the key but the LLM service layer doesn't
    wire it). The `live-anthropic` adapter will work only once that lands.
  - Provider-quality eval suite. The e2e suite tests aeterna's plumbing,
    not LLM response quality.

## Why this matters now

1. PR #169 BUILD cannot start without an answer to "what LLM does CI talk to?"
2. Solving it once, well, with a documented contract avoids ad-hoc per-test
   environment hacks.
3. The same adapter pattern is reusable for future backends (vLLM, llama.cpp
   server, Bedrock-via-mock, etc.) without re-architecting.
