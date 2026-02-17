//! Happy Remote Core Library
//!
//! Core domain types, traits, and encryption for the Happy Remote system.

// Re-export pure types from happy-types
pub use happy_types::*;

#[cfg(feature = "crypto")]
pub mod crypto;
pub mod error;
pub mod ports;

#[cfg(feature = "crypto")]
pub use crypto::NaClEngine;
pub use error::{HappyError, Result};
