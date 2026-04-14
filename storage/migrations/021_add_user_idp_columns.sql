-- Migration 021: Add IdP sync columns to users table
-- The idp-sync crate expects first_name, last_name, display_name, is_active,
-- and deactivated_at columns on the users table. These were added to the
-- initialize_github_sync_schema CREATE TABLE statement but the users table
-- already exists from migration 009 without them.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS first_name TEXT,
    ADD COLUMN IF NOT EXISTS last_name TEXT,
    ADD COLUMN IF NOT EXISTS display_name TEXT,
    ADD COLUMN IF NOT EXISTS is_active BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN IF NOT EXISTS deactivated_at TIMESTAMPTZ;

-- Back-fill is_active from existing status column for any existing rows
UPDATE users SET is_active = (status != 'inactive' AND status != 'suspended') WHERE is_active IS NULL;


