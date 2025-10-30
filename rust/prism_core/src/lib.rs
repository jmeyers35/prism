//! Core library for Prism's diff review workflow.
//!
//! The crate is layered around three primary responsibilities:
//! - repository access and snapshotting
//! - diff generation and patch application
//! - agent plugin integration for review automation
//!
//! Follow-up tasks (prism-19, prism-5, prism-24) will flesh out the modules.

#![warn(
    clippy::all,
    clippy::cargo,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]
#![cfg_attr(
    not(test),
    deny(
        clippy::dbg_macro,
        clippy::expect_used,
        clippy::panic,
        clippy::print_stderr,
        clippy::print_stdout,
        clippy::todo,
        clippy::unwrap_used
    )
)]

/// Public FFI and higher-level API surface.
pub mod api;
/// Diff generation and patching primitives.
pub mod diff;
/// Plugin registry and agent integration.
pub mod plugins;
/// Git repository access and snapshot helpers.
pub mod repository;

/// Common result type for the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors surfaced by the core library.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Placeholder variant until detailed error handling is implemented.
    #[error("operation not yet implemented: {0}")]
    Unimplemented(&'static str),
    /// Underlying git operation failed.
    #[error("git error: {source}")]
    Git {
        /// Original libgit2 error bubbled up by the core library.
        #[from]
        source: git2::Error,
    },
    /// Provided path does not correspond to a git repository.
    #[error("path does not reference a git repository: {path}")]
    NotARepository {
        /// Path that failed to resolve to a repository.
        path: String,
    },
    /// Bare repositories are currently unsupported.
    #[error("repository at {path} is bare and unsupported")]
    BareRepository {
        /// Path of the repository lacking a working tree.
        path: String,
    },
    /// Filesystem interaction failed.
    #[error("failed to access {path}: {source}")]
    Io {
        /// Filesystem path involved in the failed operation.
        path: String,
        /// Source I/O error returned by the standard library.
        #[source]
        source: std::io::Error,
    },
}
