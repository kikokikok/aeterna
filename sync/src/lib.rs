//! # Sync Bridge
//!
//! Pointer-based synchronization between memory and knowledge.

pub mod bridge;
pub mod error;
pub mod events;
pub mod pointer;
pub mod state;
pub mod state_persister;
pub mod summary_sync;

#[cfg(test)]
mod proptests;
