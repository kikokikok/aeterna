#!/usr/bin/env bash
#
# Aeterna E2E Test Runner
#
# Runs Newman against the Aeterna ingress. No port-forwards needed.
#
# Prerequisites:
#   - newman installed: npm install -g newman
#   - Optional: npm install -g newman-reporter-htmlextra
#
# Authentication (folders 13-24):
#   Folders 13-24 test authenticated workflows and require a GitHub access
#   token obtained via the OAuth device-code flow. Before running these:
#
#   1. Request a device code:
#      curl -s -X POST https://github.com/login/device/code \
#        -H 'Accept: application/json' \
#        -d 'client_id=YOUR_OAUTH_CLIENT_ID&scope=read:user,user:email'
#
#   2. Open the verification_uri in a browser and enter the user_code.
#
#   3. Poll for the token:
#      curl -s -X POST https://github.com/login/oauth/access_token \
#        -H 'Accept: application/json' \
#        -d 'client_id=YOUR_OAUTH_CLIENT_ID&device_code=DEVICE_CODE&grant_type=urn:ietf:params:oauth:grant-type:device_code'
#
#   4. Set the token in your environment file or pass it via --env-var:
#      ./run-e2e.sh --env-var "githubAccessToken=ghu_..."
#
#   To run only unauthenticated tests (folders 1-12), skip the above.
#
# Usage:
#   ./run-e2e.sh                           # Run all tests
#   ./run-e2e.sh --folder "1. Deployment"  # Run specific folder
#   ./run-e2e.sh --bail                    # Stop on first failure
#   ./run-e2e.sh --folder "13. Plugin Auth Bootstrap Flow" \
#     --env-var "githubAccessToken=ghu_..."
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COLLECTION="${SCRIPT_DIR}/aeterna-e2e.postman_collection.json"
ENVIRONMENT="${SCRIPT_DIR}/aeterna-e2e.postman_environment.json"
RESULTS_DIR="${SCRIPT_DIR}/results"

RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

log()   { echo -e "${BLUE}[e2e]${NC} $*"; }
ok()    { echo -e "${GREEN}[e2e]${NC} $*"; }
err()   { echo -e "${RED}[e2e]${NC} $*" >&2; }

check_prereqs() {
    if ! command -v newman &>/dev/null; then
        err "newman not found in PATH. Install: npm install -g newman"
        exit 1
    fi
    ok "Prerequisites OK"
}

smoke_test() {
    local base_url
    base_url=$(python3 -c "import json; env=json.load(open('${ENVIRONMENT}')); print(next(v['value'] for v in env['values'] if v['key']=='baseUrl'))" 2>/dev/null || echo "https://aeterna.example.com")

    log "Smoke test: ${base_url}/health"
    local status
    status=$(curl -s -o /dev/null -w "%{http_code}" "${base_url}/health" 2>/dev/null || echo "000")

    if [[ "$status" == "200" ]]; then
        ok "/health → 200 ✓"
    else
        err "/health → ${status} (expected 200)"
        exit 1
    fi
}

run_newman() {
    mkdir -p "$RESULTS_DIR"

    local newman_args=(
        run "$COLLECTION"
        --environment "$ENVIRONMENT"
        --timeout-request 10000
        --delay-request 200
        --color on
        --reporters cli
        --reporter-cli-no-banner
    )

    if npm list -g newman-reporter-htmlextra &>/dev/null 2>&1; then
        newman_args+=(--reporters "cli,htmlextra")
        newman_args+=(--reporter-htmlextra-export "${RESULTS_DIR}/report.html")
        log "HTML report → ${RESULTS_DIR}/report.html"
    fi

    if [[ $# -gt 0 ]]; then
        newman_args+=("$@")
    fi

    echo ""
    log "═══════════════════════════════════════════════════════════"
    log "  Aeterna E2E Tests"
    log "═══════════════════════════════════════════════════════════"
    echo ""

    local exit_code=0
    newman "${newman_args[@]}" || exit_code=$?

    echo ""
    if [[ $exit_code -eq 0 ]]; then
        ok "═══════════════════════════════════════════════════════════"
        ok "  ALL TESTS PASSED ✓"
        ok "═══════════════════════════════════════════════════════════"
    else
        err "═══════════════════════════════════════════════════════════"
        err "  TESTS FAILED (exit code: ${exit_code})"
        err "═══════════════════════════════════════════════════════════"
    fi

    [[ -f "${RESULTS_DIR}/report.html" ]] && log "HTML report: ${RESULTS_DIR}/report.html"

    return $exit_code
}

main() {
    echo ""
    log "Aeterna E2E — $(date '+%Y-%m-%d %H:%M:%S %Z')"
    echo ""
    check_prereqs
    smoke_test
    run_newman "$@"
}

main "$@"
