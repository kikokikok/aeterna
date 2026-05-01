# End-to-end testing

The `e2e/` directory contains the conformance and smoke-test assets used to
exercise Aeterna as a running system.

## LLM-backed e2e runs

The LLM-specific adapter contract lives in `e2e/llm/` and is documented in:

- `e2e/llm/README.md`
- `docs/e2e-llm-adapters.md`

Those adapters let the same e2e runner target free local inference
(`ollama`), deterministic replay (`recorded`), or hosted providers without
changing the server code under test.

## Running the suite

Use `./e2e/run-e2e.sh` for local execution, then select an LLM backend via the
adapter scripts when the test flow needs model access.
