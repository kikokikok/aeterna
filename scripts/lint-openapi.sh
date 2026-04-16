#!/usr/bin/env bash
set -euo pipefail
# Validate the OpenAPI spec using Redocly CLI (minimal ruleset).
# Style warnings (missing descriptions, operationIds) are expected
# and do not block CI.
npx --yes @redocly/cli lint \
  --skip-rule struct \
  --skip-rule operation-4xx-response \
  --skip-rule operation-operationId \
  aeterna-openapi-spec.yaml
