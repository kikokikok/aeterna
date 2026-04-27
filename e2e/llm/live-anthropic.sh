#!/usr/bin/env bash
# Live Anthropic adapter — Tier 2, local dev only. COSTS MONEY.
# NOTE: aeterna runtime does not yet route AETERNA_LLM_PROVIDER=anthropic.
# This adapter exists for forward-compat; it will work once the runtime
# factory adds Anthropic support (out of scope for this PR).
set -euo pipefail

ADAPTER_NAME=live-anthropic
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "${HERE}/_lib.sh"

MODEL="${AETERNA_E2E_LLM_MODEL:-claude-3-5-haiku-latest}"
ENDPOINT="${AETERNA_E2E_ANTHROPIC_ENDPOINT:-https://api.anthropic.com/v1}"

do_provision() {
  [[ -n "${ANTHROPIC_API_KEY:-}" ]] || die "ANTHROPIC_API_KEY not set"
  log "live-anthropic ready: ${ENDPOINT} model=${MODEL} (THIS COSTS MONEY)"
  log "WARNING: aeterna runtime does not yet wire Anthropic; runtime support is a follow-up"
}

do_env() {
  emit AETERNA_LLM_PROVIDER anthropic
  emit AETERNA_ANTHROPIC_MODEL "${MODEL}"
  emit AETERNA_ANTHROPIC_BASE_URL "${ENDPOINT}"
  emit ANTHROPIC_API_KEY "${ANTHROPIC_API_KEY:-}"
}

do_health() {
  require_cmd curl
  [[ -n "${ANTHROPIC_API_KEY:-}" ]] || die "ANTHROPIC_API_KEY not set"
  local code
  code=$(curl -sS -o /dev/null -w '%{http_code}' -m 30 "${ENDPOINT}/messages" \
    -H 'Content-Type: application/json' \
    -H "x-api-key: ${ANTHROPIC_API_KEY}" \
    -H 'anthropic-version: 2023-06-01' \
    -d "{\"model\":\"${MODEL}\",\"max_tokens\":1,\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}]}") || die "request failed"
  [[ "${code}" == "200" ]] || die "unexpected status ${code} from anthropic"
  log "health ok"
}

do_cleanup() { :; }

dispatch_subcommand "$@"
