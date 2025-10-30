//! Core library for Prism's diff review workflow.
//!
//! The crate is layered around three primary responsibilities:
//! - repository access and snapshotting
//! - diff generation and patch application
//! - agent plugin integration for review automation
//!
//! Follow-up tasks (prism-19, prism-5, prism-24) will flesh out the modules.

pub mod api;

pub mod diff;
pub mod plugins;
pub mod repository;

/// Common result type for the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors surfaced by the core library.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Placeholder variant until detailed error handling is implemented.
    #[error("operation not yet implemented: {0}")]
    Unimplemented(&'static str),
}
