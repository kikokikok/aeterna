-- Migration 035: Align organizational_units timestamp columns with Rust types
--
-- The OrganizationalUnit struct uses DateTime<Utc> for created_at / updated_at,
-- but the inline CREATE TABLE in initialize_schema() originally used BIGINT.
-- Migration 009 already defines these columns as TIMESTAMPTZ in the referential
-- DDL, so whichever ran first determined the actual column type.  This migration
-- normalises any remaining BIGINT columns to TIMESTAMPTZ, converting existing
-- epoch-seconds data with to_timestamp().

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'organizational_units'
          AND column_name = 'created_at'
          AND data_type = 'bigint'
    ) THEN
        ALTER TABLE organizational_units
            ALTER COLUMN created_at TYPE TIMESTAMPTZ
            USING to_timestamp(created_at);
        ALTER TABLE organizational_units
            ALTER COLUMN updated_at TYPE TIMESTAMPTZ
            USING to_timestamp(updated_at);
    END IF;
END $$;

-- Same fix for user_roles.created_at if it is still BIGINT.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'user_roles'
          AND column_name = 'created_at'
          AND data_type = 'bigint'
    ) THEN
        ALTER TABLE user_roles
            ALTER COLUMN created_at TYPE TIMESTAMPTZ
            USING to_timestamp(created_at);
    END IF;
END $$;
