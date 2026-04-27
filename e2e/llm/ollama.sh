#!/usr/bin/env bash
# Ollama LLM backend adapter — Tier 0 default for free, fork-safe CI.
set -euo pipefail

ADAPTER_NAME=ollama
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./_lib.sh
source "${HERE}/_lib.sh"

MODEL="${AETERNA_E2E_OLLAMA_MODEL:-qwen2.5:0.5b}"
HOST="${AETERNA_E2E_OLLAMA_HOST:-http://localhost:11434}"
CONTAINER_NAME="${AETERNA_E2E_OLLAMA_CONTAINER:-aeterna-e2e-ollama}"
PID_FILE=".e2e/ollama.started-by-us"

do_provision() {
  require_cmd curl
  mkdir -p .e2e
  if curl -fsS -m 2 "${HOST}/api/tags" >/dev/null 2>&1; then
    log "ollama already up at ${HOST} (managed externally)"
  else
    require_cmd docker
    log "starting ollama container ${CONTAINER_NAME}"
    docker rm -f "${CONTAINER_NAME}" >/dev/null 2>&1 || true
    docker run -d --name "${CONTAINER_NAME}" -p 11434:11434 \
      -v ollama-models:/root/.ollama ollama/ollama:latest >/dev/null
    touch "${PID_FILE}"
    wait_for_url "${HOST}/api/tags" 60 || die "ollama did not become ready within 60s"
  fi
  log "pulling model ${MODEL} (cached on second run)"
  curl -fsS -X POST "${HOST}/api/pull" -d "{\"name\":\"${MODEL}\",\"stream\":false}" \
    -o .e2e/ollama-pull.log || die "model pull failed; see .e2e/ollama-pull.log"
  if ! curl -fsS "${HOST}/api/tags" | grep -q "\"${MODEL}\""; then
    die "model ${MODEL} not present after pull"
  fi
  log "provision complete: ${HOST} model=${MODEL}"
}

do_env() {
  emit AETERNA_LLM_PROVIDER openai
  emit AETERNA_OPENAI_MODEL "${MODEL}"
  emit AETERNA_OPENAI_BASE_URL "${HOST}/v1"
  emit OPENAI_API_KEY ollama
}

do_health() {
  require_cmd curl
  local body
  body=$(curl -fsS -m 30 "${HOST}/v1/chat/completions" \
    -H 'Content-Type: application/json' \
    -H 'Authorization: Bearer ollama' \
    -d "{\"model\":\"${MODEL}\",\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}],\"max_tokens\":1}") || die "chat completion request failed"
  echo "${body}" | grep -q '"choices"' || die "response missing choices: ${body}"
  log "health ok"
}

do_cleanup() {
  if [[ -f "${PID_FILE}" ]]; then
    log "stopping ollama container ${CONTAINER_NAME}"
    docker rm -f "${CONTAINER_NAME}" >/dev/null 2>&1 || true
    rm -f "${PID_FILE}"
  else
    log "ollama not started by us; leaving in place"
  fi
}

dispatch_subcommand "$@"
