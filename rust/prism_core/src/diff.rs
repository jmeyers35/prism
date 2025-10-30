//! Diff generation and patch application primitives.

use crate::{Error, Result};

/// Entry point for diff generation.
#[derive(Debug, Default)]
pub struct DiffEngine;

impl DiffEngine {
    /// Construct a new diff engine instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Generate a unified diff between two snapshots.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Unimplemented`] until the diff pipeline is wired up.
    pub const fn diff(&self) -> Result<()> {
        Err(Error::Unimplemented("DiffEngine::diff"))
    }
}
