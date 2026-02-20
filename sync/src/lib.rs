//! # Sync Bridge
//!
//! Pointer-based synchronization between memory and knowledge.

pub mod bridge;
pub mod conflict;
pub mod error;
pub mod events;
pub mod live_updates;
pub mod pointer;
pub mod presence;
pub mod state;
pub mod state_persister;
pub mod summary_sync;
pub mod websocket;

#[cfg(test)]
mod proptests;
