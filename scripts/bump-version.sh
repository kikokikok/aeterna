#!/usr/bin/env bash
# scripts/bump-version.sh — single source of truth version bump.
#
# Usage: ./scripts/bump-version.sh 0.8.0-rc.1
#
# Rewrites every version string bound to the product release:
#   - Cargo workspace.package.version  (inherited by 18 crates)
#   - charts/aeterna/Chart.yaml        (version + appVersion)
#   - charts/aeterna-prereqs/Chart.yaml
#   - charts/aeterna/values.yaml       (image.tag)
#   - deploy/helm/aeterna-opal/Chart.yaml
#   - packages/opencode-plugin/package.json
#   - admin-ui/package.json
#   - website/package.json
#
# Excluded: backup/Cargo.toml (own lifecycle), .opencode/package.json (local config).

set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 <new-version>" >&2
    echo "Example: $0 0.8.0-rc.1" >&2
    exit 1
fi

NEW="$1"

if [[ ! "$NEW" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
    echo "ERROR: '$NEW' is not a valid SemVer 2.0 version" >&2
    exit 1
fi

cd "$(git rev-parse --show-toplevel)"
echo "── bumping aeterna to ${NEW} ──"

# 1. Cargo workspace
python3 - "$NEW" <<'PY'
import re, sys, pathlib
new = sys.argv[1]
p = pathlib.Path("Cargo.toml")
txt = p.read_text()
txt, n = re.subn(
    r'(\[workspace\.package\][^\[]*?version\s*=\s*")[^"]+(")',
    lambda m: f'{m.group(1)}{new}{m.group(2)}',
    txt, count=1, flags=re.DOTALL,
)
if n != 1:
    sys.exit("ERROR: failed to locate [workspace.package].version in Cargo.toml")
p.write_text(txt)
print(f"  Cargo.toml                               {new}")
PY

# 2. Helm charts
bump_chart() {
    local chart="$1"
    [[ -f "$chart" ]] || { echo "  (skip) $chart not found"; return; }
    sed -i.bak -E \
        -e "s/^version:[[:space:]]+.*/version: ${NEW}/" \
        -e "s/^appVersion:[[:space:]]+.*/appVersion: \"${NEW}\"/" \
        "$chart"
    rm "${chart}.bak"
    echo "  ${chart}   ${NEW}"
}
bump_chart "charts/aeterna/Chart.yaml"
bump_chart "charts/aeterna-prereqs/Chart.yaml"
bump_chart "deploy/helm/aeterna-opal/Chart.yaml"

# values.yaml image.tag is intentionally left empty ("") — the chart
# template falls back to .Chart.AppVersion, which bump_chart already set.
# Touching it here would hardcode the tag and defeat that design.

# 3. npm packages
# We use a targeted regex instead of `jq` to avoid reformatting unrelated
# JSON (jq normalizes arrays/objects, producing cosmetic diff noise).
# Matches only the top-level `"version": "..."` field in package.json.
bump_npm() {
    local pkgdir="$1"
    [[ -f "${pkgdir}/package.json" ]] || { echo "  (skip) ${pkgdir}/package.json not found"; return; }
    python3 - "$NEW" "${pkgdir}/package.json" <<'PY'
import re, sys, pathlib
new, path = sys.argv[1], pathlib.Path(sys.argv[2])
txt = path.read_text()
# Match the first top-level version field only (2-space indent — the
# convention in every package.json we touch). Regex anchors on the
# line-start whitespace so nested `"version":` fields (e.g. inside
# dependencies) are never matched.
txt, n = re.subn(
    r'(^\s{2}"version"\s*:\s*")[^"]+(")',
    lambda m: f'{m.group(1)}{new}{m.group(2)}',
    txt,
    count=1,
    flags=re.MULTILINE,
)
if n != 1:
    sys.exit(f"ERROR: no top-level version field found in {path}")
path.write_text(txt)
PY
    echo "  ${pkgdir}/package.json   ${NEW}"
}
bump_npm "packages/opencode-plugin"
bump_npm "admin-ui"
bump_npm "website"

# 4. Lockfiles
echo "── refreshing lockfiles ──"
cargo update --workspace --quiet 2>&1 | sed 's/^/  /' || true
for pkgdir in packages/opencode-plugin admin-ui website; do
    if [[ -f "${pkgdir}/package-lock.json" ]]; then
        ( cd "$pkgdir" && npm install --package-lock-only --silent ) && \
            echo "  ${pkgdir}/package-lock.json   refreshed"
    fi
done

# 5. Verify
echo "── verifying ──"
WS=$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
     | jq -r '.workspace_default_packages // .packages | map(select(.name != "aeterna-backup")) | .[0].version')
if [[ "$WS" != "$NEW" ]]; then
    echo "ERROR: cargo metadata reports '$WS', expected '$NEW'" >&2
    exit 1
fi

echo
echo "✅ bumped to ${NEW}. Next:"
echo "   git add -A && git commit -m 'release: ${NEW}' && git push"
echo "   after PR merges: git tag v${NEW} && git push origin v${NEW}"
