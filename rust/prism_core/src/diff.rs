//! Diff generation and patch application primitives.

use crate::{Error, Result};

/// Entry point for diff generation.
#[derive(Debug, Default)]
pub struct DiffEngine;

impl DiffEngine {
    /// Construct a new diff engine instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate a unified diff between two snapshots.
    pub fn diff(&self) -> Result<()> {
        Err(Error::Unimplemented("DiffEngine::diff"))
    }
}
