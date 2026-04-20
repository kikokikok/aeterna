# Leak-guard

Mechanical enforcement of the public/internal repository split described
in [`AGENTS.md` § Public vs Internal Repository Split](../AGENTS.md#-hard-constraint--public-vs-internal-repository-split).

## Design

Two independent checks, both against the staged diff, commit message,
and (in CI) PR title/body:

1. **Generic shape-based rules** — `.gitleaks.toml` at the repo root.
   Covers cloud provider credentials, private keys, JWTs, cloud resource
   ARNs, non-documentation IPv4 addresses, and everything the default
   [gitleaks](https://github.com/gitleaks/gitleaks) ruleset catches. This
   file is deliberately generic — it names no project-specific string.

2. **Project denylist** — a list of regexes enumerating the identifiers
   this project considers forbidden in public (environment names,
   internal hostnames, tenant slugs, cluster identifiers, etc.). This
   list is **never** committed to this repo. It lives in the internal
   repository and is loaded at scan time from one of:

   - `$AETERNA_GUARD_PATTERNS_FILE` — absolute path to a file on the
     developer's machine, typically under `~/.config/aeterna/`.
   - `$AETERNA_GUARD_PATTERNS` — inline patterns (one per line). Used
     by the CI workflow to receive the list from a GitHub Actions
     secret (`LEAK_PATTERNS`).

   Format: one extended regular expression (ERE) per line. Lines
   starting with `#` are comments; blank lines are ignored.

When a match is found, the scanner prints the offending **line number**
only, never the matched substring or the pattern. This keeps the failure
message itself leak-free.

## Local setup

One-time:

```bash
# 1. Install gitleaks
brew install gitleaks                # macOS
# or: apt install gitleaks / see https://github.com/gitleaks/gitleaks

# 2. Grab the project denylist from the internal repo and place it
#    OUTSIDE this working tree (e.g. under your user config dir).
mkdir -p ~/.config/aeterna
# copy/download the file from the internal repo — do NOT paste its
# contents anywhere that ends up in this repo.
cp /path/to/internal-repo/leak-guard/patterns.txt ~/.config/aeterna/leak-patterns.txt
chmod 600 ~/.config/aeterna/leak-patterns.txt

# 3. Point the guard at it (add to your shell rc)
echo 'export AETERNA_GUARD_PATTERNS_FILE=$HOME/.config/aeterna/leak-patterns.txt' >> ~/.zshrc

# 4. Enable the hooks in this clone
git config core.hooksPath .githooks
```

Verify:

```bash
./scripts/leak-guard.sh staged        # scans current staged diff
```

To require strict mode locally (fail instead of warn when the denylist
isn't configured):

```bash
export AETERNA_GUARD_STRICT=1
```

## CI setup

The `leak-guard` workflow runs on every push and PR. It expects a repo
secret named `LEAK_PATTERNS` containing the denylist contents (copy the
whole `patterns.txt` file into the secret value). Setting the secret is
a one-time operation by a repo admin:

```bash
gh secret set LEAK_PATTERNS < /path/to/internal-repo/leak-guard/patterns.txt
```

On fork PRs the secret is unavailable by design — the generic scan still
runs, and the project-specific scan is skipped with a warning.

## Why the denylist is not here

Writing the forbidden strings into a config file inside the public
repo would defeat the purpose: the list itself would be the leak. The
split above is load-bearing.

## Authoring guidance for the denylist (internal repo)

The internal `patterns.txt` should contain **regexes** — not literal
strings — so one line catches many related identifiers. Example shape
(the internal repo's README will show concrete entries):

```
# one regex per line, ERE
^\s*(env-name-pattern-\d+)\s*$
(?i)\binternal[-_]?project[-_]?name\b
(?i)\.example-owned-domain\.tld\b
```

Keep each pattern:

- Anchored or word-bounded (`\b`) to avoid false positives
- Case-insensitive where the real identifier's case varies
- Narrow enough that legitimate generic words (e.g. "prod", "staging",
  "cluster") don't all trip it

## Remediation on a match

1. Scrub the draft and re-stage.
2. If already committed locally, amend: `git commit --amend` after
   fixing the content and the message.
3. If already pushed, see [`AGENTS.md` § Remediation if a leak does
   ship](../AGENTS.md#remediation-if-a-leak-does-ship).
4. If a credential was matched, rotate it at its source system
   immediately — history rewrites alone do not erase credentials from
   third-party mirrors.
