//! Happy Coding - Core Library
//!
//! Core types, configuration, and adapter traits for building
//! AI coding tools across multiple platforms.

pub mod adapter;
pub mod builder;
pub mod config;
pub mod error;
pub mod types;
pub mod utils;
pub mod watcher;

pub use adapter::*;
pub use builder::*;
pub use config::*;
pub use error::*;
pub use types::*;
