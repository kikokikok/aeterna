//! # Sync Bridge
//!
//! Pointer-based synchronization between memory and knowledge.

pub mod bridge;
pub mod error;
pub mod pointer;
pub mod state;
pub mod state_persister;

#[cfg(test)]
mod proptests;
