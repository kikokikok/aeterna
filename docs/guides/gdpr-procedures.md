# GDPR Compliance Procedures

## Overview

Aeterna provides built-in functionality to comply with the General Data Protection Regulation (GDPR). This guide describes the operational procedures for managing user data requests.

## Request Intake

All GDPR requests must be recorded with:
- **Requester Name and User ID**
- **Tenant ID**
- **Type of Request** (Right to be Forgotten, Data Export, Consent Revocation)
- **Request Date**

### SLA Targets

Aeterna aims to complete all GDPR requests within **30 days**, as required by the regulation.

## Procedures

### 1. Right-to-be-Forgotten (Deletion)

This procedure hard-deletes all data associated with a user from PostgreSQL, Redis, and Qdrant.

**Step-by-Step**:
1. Verify the requester's identity.
2. Call the `delete_user_data` function in the `storage/src/gdpr.rs` API.
3. Obtain the list of `deleted_memory_ids` from the response.
4. Manually trigger the deletion of these IDs from the corresponding **Qdrant** collections if not already handled by the service wrapper.
5. Verify that no references to the user ID remain in the **Knowledge Repository**.
6. Send a confirmation notice to the requester.

### 2. Data Export (Portability)

This procedure provides the user with a copy of all their data in a machine-readable JSON format.

**Step-by-Step**:
1. Identify the user's `user_id` and `tenant_id`.
2. Call `export_user_data` from the `GdprOperations` trait.
3. Review the generated JSON for accuracy and ensure it includes:
   - All memory entries
   - Knowledge items created by the user
   - Consent records
   - Relevant audit logs (last 90 days)
4. Securely deliver the JSON file to the requester.

### 3. Anonymization

As an alternative to deletion, data can be anonymized to preserve system context without identifying individuals.

**Step-by-Step**:
1. Select the appropriate `AnonymizationStrategy`:
   - `Replace`: Replace identifiers with a fixed value.
   - `Hash`: Use a one-way cryptographic hash.
   - `Redact`: Replace with `[REDACTED]`.
2. Call `anonymize_user_data(tenant_id, user_id, strategy)`.
3. Verify that the user's content is no longer identifiable in search results.

### 4. Consent Management

Aeterna tracks user consent for different data processing purposes (e.g., "training", "sharing", "analytics").

**Procedures**:
- **Record Consent**: Call `record_consent` when a user opt-ins to a new feature or policy.
- **Review Consents**: Periodically audit `gdpr_consents` table to ensure compliance with tenant policies.
- **Revoke Consent**: Call `revoke_consent` immediately upon user request; stop processing the user's data for that purpose.

## Audit Trail

Every GDPR-related action is logged in the `gdpr_audit_logs` table.

**Audit Fields**:
- `tenant_id`
- `user_id`
- `action` (e.g., "export", "delete", "anonymize")
- `resource_type`
- `ip_address`
- `timestamp`

To retrieve a user's audit trail:
```rust
let logs = gdpr_storage.get_audit_logs(tenant_id, user_id, start_date, end_date).await?;
```

## Security & Privacy

- **Encryption**: All GDPR-related tables are protected by the same encryption-at-rest and field-level encryption policies as the core data.
- **Access Control**: Only users with the **Admin** or **DPO (Data Protection Officer)** role can execute GDPR operations.
- **Isolation**: Row Level Security (RLS) ensures that GDPR operations are strictly isolated to the specific `tenant_id`.
