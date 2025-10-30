//! Repository access and snapshot management.

use crate::{Error, Result};

/// Lightweight handle to a repository that Prism operates on.
#[derive(Debug)]
pub struct Repository;

impl Repository {
    /// Open a repository from the given filesystem path.
    pub fn open(_path: impl AsRef<std::path::Path>) -> Result<Self> {
        Err(Error::Unimplemented("Repository::open"))
    }
}
