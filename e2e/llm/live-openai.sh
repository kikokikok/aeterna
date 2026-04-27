#!/usr/bin/env bash
# Live OpenAI adapter — Tier 2, local dev / manual workflow_dispatch only. COSTS MONEY.
set -euo pipefail

ADAPTER_NAME=live-openai
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "${HERE}/_lib.sh"

MODEL="${AETERNA_E2E_LLM_MODEL:-gpt-4o-mini}"
ENDPOINT="${AETERNA_E2E_OPENAI_ENDPOINT:-https://api.openai.com/v1}"

do_provision() {
  [[ -n "${OPENAI_API_KEY:-}" ]] || die "OPENAI_API_KEY not set"
  log "live-openai ready: ${ENDPOINT} model=${MODEL} (THIS COSTS MONEY)"
}

do_env() {
  emit AETERNA_LLM_PROVIDER openai
  emit AETERNA_OPENAI_MODEL "${MODEL}"
  emit AETERNA_OPENAI_BASE_URL "${ENDPOINT}"
  emit OPENAI_API_KEY "${OPENAI_API_KEY:-}"
}

do_health() {
  require_cmd curl
  [[ -n "${OPENAI_API_KEY:-}" ]] || die "OPENAI_API_KEY not set"
  local code
  code=$(curl -sS -o /dev/null -w '%{http_code}' -m 30 "${ENDPOINT}/chat/completions" \
    -H 'Content-Type: application/json' \
    -H "Authorization: Bearer ${OPENAI_API_KEY}" \
    -d "{\"model\":\"${MODEL}\",\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}],\"max_tokens\":1}") || die "request failed"
  [[ "${code}" == "200" ]] || die "unexpected status ${code} from openai"
  log "health ok"
}

do_cleanup() { :; }

dispatch_subcommand "$@"
