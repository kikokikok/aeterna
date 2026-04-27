#!/usr/bin/env bash
# GitHub Models adapter — Tier 1, free real-LLM inference for OSS via GITHUB_TOKEN.
set -euo pipefail

ADAPTER_NAME=github-models
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "${HERE}/_lib.sh"

MODEL="${AETERNA_E2E_LLM_MODEL:-openai/gpt-4o-mini}"
ENDPOINT="${AETERNA_E2E_GITHUB_MODELS_ENDPOINT:-https://models.github.ai/inference}"

do_provision() {
  [[ -n "${GITHUB_TOKEN:-}" ]] || die "GITHUB_TOKEN not set"
  log "github-models ready: ${ENDPOINT} model=${MODEL}"
}

do_env() {
  emit AETERNA_LLM_PROVIDER openai
  emit AETERNA_OPENAI_MODEL "${MODEL}"
  emit AETERNA_OPENAI_BASE_URL "${ENDPOINT}"
  emit OPENAI_API_KEY "${GITHUB_TOKEN:-}"
}

do_health() {
  require_cmd curl
  [[ -n "${GITHUB_TOKEN:-}" ]] || die "GITHUB_TOKEN not set"
  local code
  code=$(curl -sS -o /dev/null -w '%{http_code}' -m 30 "${ENDPOINT}/chat/completions" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer ${GITHUB_TOKEN}" \
    -d "{\"model\":\"${MODEL}\",\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}],\"max_tokens\":1}") || die "request failed"
  [[ "${code}" == "200" ]] || die "unexpected status ${code} from github-models"
  log "health ok"
}

do_cleanup() { :; }

dispatch_subcommand "$@"
