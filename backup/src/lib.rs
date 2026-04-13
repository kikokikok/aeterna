//! Aeterna backup/restore system.
//!
//! This crate provides the core archive format, streaming NDJSON
//! serialization, SHA-256 integrity verification, and offline validation
//! for Aeterna backup archives (`.tar.gz` bundles).
//!
//! # Modules
//!
//! - [`manifest`] -- Archive metadata, entity counts, and backend snapshot IDs.
//! - [`ndjson`] -- Streaming newline-delimited JSON reader and writer.
//! - [`checksum`] -- SHA-256 computation and verification utilities.
//! - [`archive`] -- Tar/gzip archive writer and reader.
//! - [`validate`] -- Offline archive validation without restoring.
//! - [`error`] -- Structured error types for backup operations.

pub mod archive;
pub mod checksum;
pub mod destination;
pub mod error;
pub mod manifest;
pub mod ndjson;
pub mod s3;
pub mod validate;
