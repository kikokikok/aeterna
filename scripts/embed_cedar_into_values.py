#!/usr/bin/env python3
"""Embed Cedar policy + schema text into a Helm values.yaml.

Used by the helm packaging step of release.yml (and formerly helm-release.yml).
Replaces the inline here-doc so the logic is unit-testable.

Usage: embed_cedar_into_values.py <values.yaml> <policy.txt> <schema.txt>
"""
import sys


def main() -> int:
    if len(sys.argv) != 4:
        print("usage: embed_cedar_into_values.py <values.yaml> <policy.txt> <schema.txt>", file=sys.stderr)
        return 2

    values_path, policy_path, schema_path = sys.argv[1:]

    with open(policy_path) as f:
        policy_text = f.read()
    with open(schema_path) as f:
        schema_text = f.read()
    with open(values_path) as f:
        content = f.read()

    lines = content.split("\n")
    out: list[str] = []
    for i, line in enumerate(lines):
        stripped = line.lstrip()
        window = "\n".join(lines[max(0, i - 5) : i])
        if stripped.startswith("policyText:") and "cedar:" in window:
            indent = " " * (len(line) - len(stripped))
            out.append(f"{indent}policyText: |")
            out.extend(f"{indent}  {pline}" for pline in policy_text.rstrip("\n").split("\n"))
        elif stripped.startswith("schemaText:") and "cedar:" in window:
            indent = " " * (len(line) - len(stripped))
            out.append(f"{indent}schemaText: |")
            out.extend(f"{indent}  {sline}" for sline in schema_text.rstrip("\n").split("\n"))
        else:
            out.append(line)

    with open(values_path, "w") as f:
        f.write("\n".join(out))

    print(f"Cedar policies embedded: {len(policy_text)} bytes policy, {len(schema_text)} bytes schema")
    return 0


if __name__ == "__main__":
    sys.exit(main())
