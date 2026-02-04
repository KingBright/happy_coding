//! Happy Coding - Core Library
//!
//! Core types, configuration, and adapter traits for building
//! AI coding tools across multiple platforms.

pub mod types;
pub mod config;
pub mod adapter;
pub mod builder;
pub mod watcher;
pub mod error;

pub use types::*;
pub use config::*;
pub use adapter::*;
pub use builder::*;
pub use error::*;
