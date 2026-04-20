#!/usr/bin/env bash
# leak-guard.sh
#
# Runs two checks against a given scope (staged diff, commit range,
# file, or PR/issue body text):
#
#   1. gitleaks with the in-repo .gitleaks.toml (generic shapes only).
#   2. A project-specific denylist loaded from a private source.
#      The denylist file is ONE regex per line (POSIX ERE), lines
#      starting with '#' are comments, blank lines ignored. This file
#      is NEVER stored in the public repo. Source priority:
#
#         a) $AETERNA_GUARD_PATTERNS_FILE (path to file outside repo)
#         b) $AETERNA_GUARD_PATTERNS (patterns inline, one per line)
#         c) GitHub Actions secret LEAK_PATTERNS (see CI workflow)
#
#      If none is configured, the project-specific scan is SKIPPED
#      with a warning. CI is configured to fail instead of skipping;
#      local dev defaults to warn-and-continue unless
#      AETERNA_GUARD_STRICT=1.
#
# Exits non-zero on the first match. Never prints the pattern that
# matched, only the offending path + line number. The matching line
# itself is printed ONLY when --show-lines is passed.

set -euo pipefail

usage() {
  cat <<USAGE
Usage: leak-guard.sh <mode> [args...]

Modes:
  staged                       Scan the current git staged diff + HEAD commit message.
  diff <base> <head>           Scan the diff between two refs.
  file <path>                  Scan a single file's contents.
  text                         Scan stdin as free text (use for PR/issue bodies).

Environment:
  AETERNA_GUARD_PATTERNS_FILE  Path to private denylist file (preferred).
  AETERNA_GUARD_PATTERNS       Inline denylist (newline-separated).
  AETERNA_GUARD_STRICT=1       Fail instead of warning when no denylist is configured.
  AETERNA_GUARD_SKIP_GITLEAKS=1  Skip the gitleaks step (denylist only).
USAGE
}

log()  { printf '[leak-guard] %s\n' "$*" >&2; }
fail() { printf '[leak-guard] ❌ %s\n' "$*" >&2; exit 1; }
warn() { printf '[leak-guard] ⚠️  %s\n' "$*" >&2; }

[[ $# -ge 1 ]] || { usage >&2; exit 2; }
MODE=$1; shift

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
cd "$REPO_ROOT"

# ---- prepare a temp file holding the payload to scan ------------
PAYLOAD=$(mktemp -t leakguard.XXXXXX)
trap 'rm -f "$PAYLOAD" "$PATTERNS_FILE_INTERNAL" 2>/dev/null || true' EXIT

case "$MODE" in
  staged)
    git diff --cached > "$PAYLOAD"
    # Also scan the prepared-commit-msg if the hook passes it.
    if [[ -f "$REPO_ROOT/.git/COMMIT_EDITMSG" ]]; then
      printf '\n--- commit message ---\n' >> "$PAYLOAD"
      cat "$REPO_ROOT/.git/COMMIT_EDITMSG" >> "$PAYLOAD"
    fi
    ;;
  diff)
    [[ $# -eq 2 ]] || { usage >&2; exit 2; }
    git diff "$1" "$2" > "$PAYLOAD"
    git log --format='%B%n' "$1".. "$2" >> "$PAYLOAD" || true
    ;;
  file)
    [[ $# -eq 1 ]] || { usage >&2; exit 2; }
    cat "$1" > "$PAYLOAD"
    ;;
  text)
    cat > "$PAYLOAD"
    ;;
  *)
    usage >&2; exit 2
    ;;
esac

# ---- step 1: gitleaks (generic shapes) --------------------------
if [[ "${AETERNA_GUARD_SKIP_GITLEAKS:-0}" != "1" ]]; then
  if command -v gitleaks >/dev/null 2>&1; then
    if ! gitleaks detect \
        --no-banner \
        --redact \
        --no-git \
        --source="$PAYLOAD" \
        --config="$REPO_ROOT/.gitleaks.toml" \
        >/dev/null 2>&1; then
      # Re-run with output visible so user sees redacted findings.
      gitleaks detect --no-banner --redact --no-git \
          --source="$PAYLOAD" \
          --config="$REPO_ROOT/.gitleaks.toml" >&2 || true
      fail "generic secret / shape match — scrub before committing"
    fi
  else
    warn "gitleaks not installed; skipping generic-shape scan (install: https://github.com/gitleaks/gitleaks)"
  fi
fi

# ---- step 2: project denylist (private, never in-repo) ----------
PATTERNS_FILE_INTERNAL=""
if [[ -n "${AETERNA_GUARD_PATTERNS_FILE:-}" && -r "$AETERNA_GUARD_PATTERNS_FILE" ]]; then
  PATTERNS_FILE_INTERNAL=$AETERNA_GUARD_PATTERNS_FILE
elif [[ -n "${AETERNA_GUARD_PATTERNS:-}" ]]; then
  PATTERNS_FILE_INTERNAL=$(mktemp -t leakpatterns.XXXXXX)
  printf '%s\n' "$AETERNA_GUARD_PATTERNS" > "$PATTERNS_FILE_INTERNAL"
fi

if [[ -z "$PATTERNS_FILE_INTERNAL" ]]; then
  if [[ "${AETERNA_GUARD_STRICT:-0}" = "1" ]]; then
    fail "no project denylist configured (set AETERNA_GUARD_PATTERNS_FILE or AETERNA_GUARD_PATTERNS); see docs/leak-guard.md"
  fi
  warn "no project denylist configured — generic scan ran, project-specific scan SKIPPED"
  warn "(configure AETERNA_GUARD_PATTERNS_FILE per docs/leak-guard.md)"
  exit 0
fi

# Strip comments / blanks and run grep with -f (patterns from file).
# Output format: we deliberately do NOT echo the matched substring,
# only the file-synthetic line number from the payload, so a leaking
# commit message doesn't also echo its own leak on failure.
CLEAN_PATTERNS=$(mktemp -t leakpatternsC.XXXXXX)
grep -vE '^\s*(#|$)' "$PATTERNS_FILE_INTERNAL" > "$CLEAN_PATTERNS" || true
trap 'rm -f "$PAYLOAD" "$PATTERNS_FILE_INTERNAL" "$CLEAN_PATTERNS" 2>/dev/null || true' EXIT

if [[ ! -s "$CLEAN_PATTERNS" ]]; then
  warn "denylist is empty after comment stripping; nothing to scan"
  exit 0
fi

MATCH_COUNT=$(grep -cEHnf "$CLEAN_PATTERNS" "$PAYLOAD" 2>/dev/null | awk -F: '{n+=$2} END {print n+0}')
# grep -c returns one count per input file; with a single file it's just that number.
# Re-run to get the line numbers (without the matched content).
if grep -nEf "$CLEAN_PATTERNS" "$PAYLOAD" >/dev/null 2>&1; then
  LINES=$(grep -nEf "$CLEAN_PATTERNS" "$PAYLOAD" | awk -F: '{print $1}' | paste -sd',' -)
  fail "project-denylist match(es) at payload line(s): $LINES — scrub before committing (the pattern itself is not printed on purpose)"
fi

log "✅ clean (generic + project denylist)"
