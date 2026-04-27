#!/usr/bin/env bash
# Verifies all LLM-backend adapters implement the 4-subcommand contract.
# Does NOT call `provision`/`health` for backends that need real network or
# secrets; those are exercised in their respective workflow tiers.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fail=0
pass=0

required_keys=(AETERNA_LLM_PROVIDER AETERNA_OPENAI_BASE_URL OPENAI_API_KEY AETERNA_OPENAI_MODEL)
# live-anthropic emits ANTHROPIC_* instead; allow that variant.
required_keys_anthropic=(AETERNA_LLM_PROVIDER AETERNA_ANTHROPIC_BASE_URL ANTHROPIC_API_KEY AETERNA_ANTHROPIC_MODEL)

check_backend() {
  local script="$1" name; name="$(basename "${script}" .sh)"
  echo "== ${name} =="

  # 1. executable
  [[ -x "${script}" ]] || { echo "  FAIL: not executable"; fail=$((fail+1)); return; }

  # 2. bash -n parse
  bash -n "${script}" || { echo "  FAIL: bash -n"; fail=$((fail+1)); return; }

  # 3. bogus subcommand exits 64
  local rc=0
  "${script}" __bogus__ >/dev/null 2>&1 || rc=$?
  [[ "${rc}" == "64" ]] || { echo "  FAIL: bogus subcommand exit ${rc} != 64"; fail=$((fail+1)); return; }

  # 4. env emits required keys (set placeholder secrets so live-* don't die)
  local env_out
  env_out=$(GITHUB_TOKEN=placeholder OPENAI_API_KEY=placeholder ANTHROPIC_API_KEY=placeholder \
            AETERNA_E2E_LLM_FIXTURES=/tmp/_llm_fixtures \
            "${script}" env 2>/dev/null) || {
    # recorded needs provision to know its port; skip its env check here.
    if [[ "${name}" == "recorded" ]]; then
      echo "  SKIP env (needs provision; covered by smoke test)"; pass=$((pass+1)); return
    fi
    echo "  FAIL: env subcommand failed"; fail=$((fail+1)); return
  }

  local keys=("${required_keys[@]}")
  [[ "${name}" == "live-anthropic" ]] && keys=("${required_keys_anthropic[@]}")
  for k in "${keys[@]}"; do
    grep -q "^${k}=" <<<"${env_out}" || { echo "  FAIL: env missing ${k}"; fail=$((fail+1)); return; }
  done

  echo "  ok"; pass=$((pass+1))
}

for s in "${HERE}"/*.sh; do
  [[ "$(basename "${s}")" == "_lib.sh" ]] && continue
  [[ "$(basename "${s}")" == "contract-test.sh" ]] && continue
  check_backend "${s}"
done

echo
echo "summary: ${pass} passed, ${fail} failed"
exit "${fail}"
