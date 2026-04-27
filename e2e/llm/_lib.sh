#!/usr/bin/env bash
# Shared helpers for LLM-backend adapter scripts. Sourced, never executed.
# shellcheck shell=bash

set -euo pipefail

log() { printf '[%s] %s\n' "${ADAPTER_NAME:-llm}" "$*" >&2; }
die() { log "ERROR: $*"; exit 1; }
usage() { printf 'usage: %s {provision|env|health|cleanup}\n' "$0" >&2; exit 64; }

require_cmd() {
  for c in "$@"; do command -v "$c" >/dev/null 2>&1 || die "missing required command: $c"; done
}

# Wait for a URL to return HTTP 2xx/3xx. Args: <url> [timeout-seconds]
wait_for_url() {
  local url="$1" timeout="${2:-60}" elapsed=0
  while (( elapsed < timeout )); do
    if curl -fsS -o /dev/null -m 2 "$url" 2>/dev/null; then return 0; fi
    sleep 1; elapsed=$((elapsed + 1))
  done
  return 1
}

# Pick a free localhost port.
random_port() {
  python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()'
}

# Print env line, escaping is the caller's responsibility (we keep values simple).
emit() { printf '%s=%s\n' "$1" "$2"; }

# Dispatch helper: each adapter calls `dispatch_subcommand "$@"` after defining
# do_provision / do_env / do_health / do_cleanup.
dispatch_subcommand() {
  local sub="${1:-}"; shift || true
  case "$sub" in
    provision) do_provision "$@" ;;
    env)       do_env "$@" ;;
    health)    do_health "$@" ;;
    cleanup)   do_cleanup "$@" ;;
    *)         usage ;;
  esac
}
