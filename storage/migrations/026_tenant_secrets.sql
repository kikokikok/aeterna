-- Unified tenant secret storage.
--
-- Replaces two parallel systems that previously held secret material:
--   1. storage/src/secret_provider.rs (git-token focused, with stub AWS/Vault
--      backends that were never wired).
--   2. storage/src/tenant_config_provider.rs::KubernetesTenantConfigProvider
--      (in-memory HashMap, lost on restart).
--
-- Design: envelope encryption.
--   - A fresh 32-byte data encryption key (DEK) is generated per write.
--   - Secret bytes are encrypted with the DEK using AES-256-GCM (authenticated).
--   - The DEK itself is encrypted ("wrapped") by a KMS Customer Master Key
--     (CMK) and the wrapped ciphertext is persisted in `wrapped_dek`.
--   - At read time, the service calls KMS.Decrypt(wrapped_dek) to recover the
--     DEK, then AES-GCM decrypts the row ciphertext.
--
-- Rotation: to rotate the CMK, a background job may decrypt each row's
-- wrapped_dek with the old CMK and re-encrypt with the new one. The row
-- ciphertext itself is untouched. `kms_key_id` records which CMK wrapped
-- the current DEK so the rotation job can target old rows precisely.
--
-- See: openspec/changes/harden-tenant-provisioning/design.md (D2, D4)

CREATE TABLE IF NOT EXISTS tenant_secrets (
    id             UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id      UUID         NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,

    -- Human-readable name used in manifests, e.g. "llm_api_key",
    -- "embedding_api_key", or (for migrated git tokens)
    -- "git_token:<connection_id>".
    logical_name   TEXT         NOT NULL,

    -- KMS handle that wrapped the current DEK. For AWS this is the key ARN
    -- (or alias ARN) used at encrypt time. Kept for observability and for
    -- future CMK rotation jobs; not required at decrypt time (the wrapped
    -- DEK blob is self-describing for AWS KMS).
    kms_key_id     TEXT         NOT NULL,

    -- Wrapped DEK: KMS.Encrypt(plaintext=DEK_bytes, key=kms_key_id).
    -- Opaque to us; never decrypt this except through the configured KmsProvider.
    wrapped_dek    BYTEA        NOT NULL,

    -- AES-256-GCM ciphertext of the actual secret bytes, using the DEK.
    ciphertext     BYTEA        NOT NULL,

    -- 12-byte AES-GCM nonce used to encrypt `ciphertext`.
    nonce          BYTEA        NOT NULL,

    -- Bumped on every update. The application enforces strict monotonicity.
    generation     BIGINT       NOT NULL DEFAULT 1,

    created_at     TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ  NOT NULL DEFAULT NOW(),

    -- One logical name per tenant. Updates mutate the row in place rather
    -- than inserting a new one so SecretReference::Postgres { secret_id }
    -- remains stable across rotations.
    UNIQUE (tenant_id, logical_name)
);

CREATE INDEX IF NOT EXISTS idx_tenant_secrets_tenant
    ON tenant_secrets (tenant_id);

-- Touch updated_at on every UPDATE.
CREATE OR REPLACE FUNCTION tenant_secrets_touch_updated_at()
    RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at := NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_tenant_secrets_touch_updated_at ON tenant_secrets;
CREATE TRIGGER trg_tenant_secrets_touch_updated_at
    BEFORE UPDATE ON tenant_secrets
    FOR EACH ROW
    EXECUTE FUNCTION tenant_secrets_touch_updated_at();
