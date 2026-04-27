#!/usr/bin/env bash
# Recorded-fixture replay adapter — Tier 0 fast path, $0, deterministic.
set -euo pipefail

ADAPTER_NAME=recorded
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${HERE}/../.." && pwd)"
# shellcheck source=./_lib.sh
source "${HERE}/_lib.sh"

MODE="${AETERNA_E2E_LLM_RECORDED_MODE:-replay}"
FIXTURES="${AETERNA_E2E_LLM_FIXTURES:-${ROOT}/e2e/fixtures/llm}"
PID_FILE="${ROOT}/.e2e/llm-recorded.pid"
PORT_FILE="${ROOT}/.e2e/llm-recorded.port"
LOG_FILE="${ROOT}/.e2e/llm-recorded.log"
MODEL="${AETERNA_E2E_LLM_MODEL:-recorded-fixture}"

do_provision() {
  require_cmd python3 curl
  mkdir -p "${ROOT}/.e2e"
  if [[ -f "${PID_FILE}" ]] && kill -0 "$(cat "${PID_FILE}")" 2>/dev/null; then
    log "server already running (pid $(cat "${PID_FILE}"))"
    return 0
  fi
  local port; port=$(random_port)
  echo "${port}" > "${PORT_FILE}"
  local upstream_args=()
  if [[ "${MODE}" == "record" ]]; then
    local up="${AETERNA_E2E_LLM_RECORD_UPSTREAM:-}"
    case "${up}" in
      live-openai)    upstream_args=(--upstream-url "https://api.openai.com/v1" --upstream-key "${OPENAI_API_KEY:-}") ;;
      github-models)  upstream_args=(--upstream-url "https://models.github.ai/inference" --upstream-key "${GITHUB_TOKEN:-}") ;;
      live-anthropic) die "recording from anthropic not supported (response shape differs)" ;;
      *) die "AETERNA_E2E_LLM_RECORD_UPSTREAM must be one of: live-openai, github-models" ;;
    esac
  fi
  log "starting mock server on :${port} mode=${MODE} fixtures=${FIXTURES}"
  nohup python3 "${ROOT}/e2e/tools/mock-llm-server.py" \
    --port "${port}" \
    --fixtures "${FIXTURES}" \
    --mode "${MODE}" \
    "${upstream_args[@]}" \
    > "${LOG_FILE}" 2>&1 &
  echo $! > "${PID_FILE}"
  wait_for_url "http://127.0.0.1:${port}/healthz" 10 \
    || { cat "${LOG_FILE}" >&2; die "mock server did not become ready"; }
  log "ready (pid $(cat "${PID_FILE}"))"
}

do_env() {
  local port; port=$(cat "${PORT_FILE}" 2>/dev/null || echo "")
  [[ -n "${port}" ]] || die "server not provisioned"
  emit AETERNA_LLM_PROVIDER openai
  emit AETERNA_OPENAI_MODEL "${MODEL}"
  emit AETERNA_OPENAI_BASE_URL "http://127.0.0.1:${port}/v1"
  emit OPENAI_API_KEY recorded
}

do_health() {
  local port; port=$(cat "${PORT_FILE}" 2>/dev/null || echo "")
  [[ -n "${port}" ]] || die "server not provisioned"
  curl -fsS -m 5 "http://127.0.0.1:${port}/healthz" >/dev/null || die "healthz failed"
  log "health ok"
}

do_cleanup() {
  if [[ -f "${PID_FILE}" ]]; then
    local pid; pid=$(cat "${PID_FILE}")
    kill "${pid}" 2>/dev/null || true
    rm -f "${PID_FILE}" "${PORT_FILE}"
    log "stopped (pid ${pid})"
  fi
}

dispatch_subcommand "$@"
