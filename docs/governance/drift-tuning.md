# Drift Detection Tuning Guide

This guide explains how to configure and tune drift detection in the Aeterna governance system for optimal accuracy and reduced noise.

## Overview

Drift detection identifies when project code or configurations deviate from established policies. However, not all detected drifts are actionable—some are false positives, expected exceptions, or low-confidence results that require manual review.

The drift tuning system provides:
- **Suppression rules** to filter out known exceptions
- **Confidence scoring** to identify uncertain detections
- **Configurable thresholds** per project
- **Manual review flagging** for ambiguous cases

## Concepts

### Drift Score

A value between 0.0 and 1.0 indicating the degree of policy deviation:

| Score Range | Interpretation |
|-------------|----------------|
| 0.0 - 0.2 | No significant drift |
| 0.2 - 0.5 | Minor drift, review recommended |
| 0.5 - 0.8 | Moderate drift, action needed |
| 0.8 - 1.0 | Severe drift, immediate attention |

### Confidence Score

A value between 0.0 and 1.0 indicating how certain the system is about the drift detection:

| Score Range | Interpretation |
|-------------|----------------|
| 0.9 - 1.0 | High confidence (rule-based detection) |
| 0.75 - 0.9 | Medium confidence (semantic analysis) |
| 0.0 - 0.75 | Low confidence (requires manual review) |

### Suppression Rules

Rules that filter out expected or known violations from drift reports. Useful for:
- Legacy code with approved exceptions
- Third-party dependencies with known patterns
- Environment-specific configurations

## Configuration

### Per-Project Drift Threshold

Configure the drift threshold for each project:

```bash
# Set drift threshold via API
curl -X PUT "http://localhost:8080/api/v1/governance/drift-config/my-project" \
  -H "Content-Type: application/json" \
  -H "X-Tenant-Id: acme-corp" \
  -d '{
    "threshold": 0.3,
    "auto_suppress_info": true
  }'
```

#### Configuration Options

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `threshold` | f32 | 0.2 | Drift score threshold for alerts |
| `auto_suppress_info` | bool | false | Automatically suppress INFO severity violations |

### Suppression Rules

Create suppression rules to filter known exceptions:

```bash
# Create a suppression rule
curl -X POST "http://localhost:8080/api/v1/governance/suppressions" \
  -H "Content-Type: application/json" \
  -H "X-Tenant-Id: acme-corp" \
  -d '{
    "project_id": "my-project",
    "policy_id": "security-policy",
    "rule_pattern": "lodash.*version",
    "reason": "Legacy dependency approved in ADR-042",
    "expires_at": 1735689600
  }'
```

#### Suppression Rule Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `project_id` | String | Yes | Project to apply suppression to |
| `policy_id` | String | Yes | Policy ID whose violations to suppress |
| `rule_pattern` | String | No | Regex pattern to match violation messages |
| `reason` | String | Yes | Explanation for the suppression |
| `expires_at` | i64 | No | Unix timestamp when suppression expires |

### List Suppressions

```bash
# List all suppressions for a project
curl "http://localhost:8080/api/v1/governance/suppressions/my-project" \
  -H "X-Tenant-Id: acme-corp"
```

Response:
```json
{
  "suppressions": [
    {
      "id": "sup-123",
      "project_id": "my-project",
      "policy_id": "security-policy",
      "rule_pattern": "lodash.*version",
      "reason": "Legacy dependency approved in ADR-042",
      "created_at": 1704067200,
      "expires_at": 1735689600
    }
  ]
}
```

### Delete Suppression

```bash
curl -X DELETE "http://localhost:8080/api/v1/governance/suppressions/sup-123" \
  -H "X-Tenant-Id: acme-corp"
```

## Confidence Scoring

### How Confidence is Calculated

The drift detection system assigns confidence scores based on the detection method:

| Detection Method | Base Confidence | Rationale |
|------------------|-----------------|-----------|
| Rule-based (exact match) | 1.0 | Deterministic pattern matching |
| Semantic (embedding similarity) | 0.85 | Vector similarity may have false positives |
| LLM-based (reasoning) | 0.75 | Model interpretation can vary |

### Low Confidence Handling

When confidence drops below 0.7, the drift result is flagged for manual review:

```rust
pub struct DriftResult {
    pub project_id: String,
    pub drift_score: f32,
    pub confidence: f32,               // 0.0 - 1.0
    pub violations: Vec<PolicyViolation>,
    pub suppressed_violations: Vec<PolicyViolation>,
    pub requires_manual_review: bool,  // true if confidence < 0.7
    pub timestamp: i64,
}
```

### Manual Review Workflow

1. Weekly drift reports identify items requiring manual review
2. Reviewers assess flagged violations
3. Legitimate violations are addressed; false positives become suppression rules

## Drift Reports

Weekly drift reports now include tuning information:

```
Weekly Governance Report - 2025-01-15
=====================================

Summary:
- Total Projects Analyzed: 42
- Active Violations: 15
- Suppressed Violations: 8
- Manual Review Required: 3

Projects Requiring Attention:
- payments-service: drift=0.45, confidence=0.92, violations=5
- auth-service: drift=0.32, confidence=0.68 (REVIEW)
- gateway-api: drift=0.28, confidence=0.95, violations=2
```

### Report Fields

| Field | Description |
|-------|-------------|
| `active_violation_count` | Violations not suppressed |
| `suppressed_violation_count` | Violations filtered by suppression rules |
| `manual_review_required` | Count of low-confidence results needing review |

## API Reference

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/governance/suppressions` | Create suppression rule |
| `GET` | `/api/v1/governance/suppressions/{project_id}` | List suppressions |
| `DELETE` | `/api/v1/governance/suppressions/{suppression_id}` | Delete suppression |
| `GET` | `/api/v1/governance/drift-config/{project_id}` | Get drift config |
| `PUT` | `/api/v1/governance/drift-config/{project_id}` | Update drift config |

### Data Types

#### DriftConfig

```rust
pub struct DriftConfig {
    pub project_id: String,
    pub threshold: f32,          // Default: 0.2
    pub auto_suppress_info: bool, // Default: false
}
```

#### DriftSuppression

```rust
pub struct DriftSuppression {
    pub id: String,
    pub project_id: String,
    pub tenant_id: TenantId,
    pub policy_id: String,
    pub rule_pattern: Option<String>,
    pub reason: String,
    pub created_at: i64,
    pub expires_at: Option<i64>,
}
```

## Best Practices

### 1. Start with Conservative Thresholds

Begin with the default threshold (0.2) and adjust based on:
- Volume of alerts
- Team capacity to review
- Historical drift patterns

### 2. Document Suppression Reasons

Always provide clear reasons for suppressions linking to:
- ADR approving the exception
- Ticket tracking the remediation
- Timeline for addressing the drift

### 3. Set Expiration Dates

Don't create permanent suppressions. Set expiration dates to force periodic review:

```bash
# Suppression expires in 90 days
"expires_at": $(date -d "+90 days" +%s)
```

### 4. Review Low-Confidence Results

Establish a weekly review process for items flagged as `requires_manual_review`:
- True positives: Create tickets to address
- False positives: Create suppression rules with patterns

### 5. Monitor Suppression Growth

Track the ratio of suppressed to active violations. High suppression counts may indicate:
- Policies too strict for the codebase
- Technical debt accumulation
- Need for policy refinement

## Troubleshooting

### High False Positive Rate

**Symptoms**: Many violations that aren't actionable

**Solutions**:
1. Review policy rule specificity
2. Add targeted suppression rules with patterns
3. Adjust semantic similarity thresholds
4. Enable `auto_suppress_info` for informational violations

### Low Confidence Scores

**Symptoms**: Many results requiring manual review

**Solutions**:
1. Improve policy rule definitions (more specific patterns)
2. Review embedding model quality
3. Consider rule-based detection for critical policies

### Suppression Rules Not Matching

**Symptoms**: Violations not being suppressed despite rules

**Solutions**:
1. Check regex pattern syntax
2. Verify `policy_id` matches exactly
3. Check suppression hasn't expired
4. Ensure `project_id` matches

## Migration from Previous Versions

If upgrading from a version without drift tuning:

1. Run migration `005_drift_tuning.sql`:
   ```bash
   psql -d aeterna -f storage/migrations/005_drift_tuning.sql
   ```

2. Existing drift results will have:
   - `confidence = 1.0` (assumed high confidence)
   - `suppressed_violations = []` (no suppressions)
   - `requires_manual_review = false`

3. Create suppression rules for known exceptions

4. Adjust thresholds based on current drift levels

## Related Documentation

- [API Reference](api-reference.md) - Full governance API documentation
- [Policy Model](policy-model.md) - Understanding policies and rules
- [Troubleshooting](troubleshooting.md) - Common issues and solutions
- [Deployment Guide](deployment-guide.md) - Production deployment
