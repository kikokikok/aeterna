#!/usr/bin/env bash
# B2 §13.7 — fast structural pre-check across every consistency-suite
# fixture. Runs WITHOUT a live Aeterna server.
#
# This guards the fixtures themselves:
#   1. Every `scenarios/*.json` file is parseable JSON.
#   2. Every file declares the canonical `apiVersion` and `kind`.
#   3. Every file has a non-empty `tenant.slug` and `tenant.name`.
#   4. Every file carries `metadata.labels.suite == "consistency"`
#      so accidental drive-by edits to unrelated manifests can't
#      smuggle themselves into the suite run.
#
# The full server-backed validation (`POST
# /admin/tenants/provision?dryRun=true`) is the job of the per-runner
# tests (§13.2–§13.4); this script stays client-local so it can run
# in the fastest possible CI lane and in pre-commit hooks.
#
# Usage:
#   ./run_validate.sh
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
SCENARIOS_DIR="$SCRIPT_DIR/scenarios"

if ! command -v jq >/dev/null 2>&1; then
  echo "❌ jq not found on PATH; install it (brew install jq / apt-get install jq)" >&2
  exit 2
fi

fixtures=()
while IFS= read -r -d '' f; do fixtures+=("$f"); done \
  < <(find "$SCENARIOS_DIR" -maxdepth 1 -type f -name '*.json' -print0 | sort -z)

if [ "${#fixtures[@]}" -eq 0 ]; then
  echo "❌ no fixtures found under $SCENARIOS_DIR" >&2
  exit 2
fi

failures=0
for fixture in "${fixtures[@]}"; do
  name="$(basename "$fixture")"
  errors=()

  # 1. Parseable JSON.
  if ! jq -e . "$fixture" >/dev/null 2>&1; then
    errors+=("not valid JSON")
    echo "❌ $name"
    printf '   - %s\n' "${errors[@]}"
    failures=$((failures + 1))
    continue
  fi

  # 2. Canonical apiVersion + kind.
  av="$(jq -r '.apiVersion // empty' "$fixture")"
  if [ "$av" != "aeterna.io/v1" ]; then
    errors+=("apiVersion must be 'aeterna.io/v1', got: '$av'")
  fi
  k="$(jq -r '.kind // empty' "$fixture")"
  if [ "$k" != "TenantManifest" ]; then
    errors+=("kind must be 'TenantManifest', got: '$k'")
  fi

  # 3. Non-empty tenant.slug + tenant.name.
  slug="$(jq -r '.tenant.slug // empty' "$fixture")"
  if [ -z "$slug" ]; then
    errors+=("tenant.slug is missing or empty")
  fi
  tname="$(jq -r '.tenant.name // empty' "$fixture")"
  if [ -z "$tname" ]; then
    errors+=("tenant.name is missing or empty")
  fi

  # 4. Suite membership label.
  suite="$(jq -r '.metadata.labels.suite // empty' "$fixture")"
  if [ "$suite" != "consistency" ]; then
    errors+=("metadata.labels.suite must be 'consistency', got: '$suite'")
  fi

  if [ "${#errors[@]}" -eq 0 ]; then
    echo "✅ $name (slug=$slug)"
  else
    echo "❌ $name"
    printf '   - %s\n' "${errors[@]}"
    failures=$((failures + 1))
  fi
done

if [ "$failures" -gt 0 ]; then
  echo ""
  echo "❌ $failures fixture(s) failed structural validation" >&2
  exit 1
fi

echo ""
echo "✅ all ${#fixtures[@]} fixtures passed structural validation"
