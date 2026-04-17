-- Migration 022: Drop dead VECTOR columns
--
-- Background:
--   Earlier migrations (003, 007) declared VECTOR(1536) columns on
--   `memory_entries` (`embedding`) and via ALTER on `memory_entries`
--   (`context_vector`). These columns were never read or written by Rust
--   code — semantic vectors live in Qdrant (see `memory::backends::qdrant`).
--   Their presence forced the deployment to install the pgvector extension
--   and made stock Postgres containers unusable in tests.
--
-- This migration:
--   1. Drops both dead columns if present (idempotent via IF EXISTS).
--   2. Allows the pgvector extension to be optional / removed from the
--      prereq chart.
--
-- Migrations 003 and 007 have been rewritten in the tree to match this
-- post-cleanup schema. Environments bootstrapped before this change will
-- see checksum mismatches on those versions; remediate by wiping the
-- affected dev DB (prod is empty) or by manually fixing
-- `_aeterna_migrations` checksums.

ALTER TABLE memory_entries DROP COLUMN IF EXISTS embedding;
ALTER TABLE memory_entries DROP COLUMN IF EXISTS context_vector;
