use std::sync::PoisonError;

use thiserror::Error;

use crate::Error;

/// Errors surfaced through the `UniFFI` bindings.
#[derive(Debug, Error)]
pub enum CoreError {
    /// Path does not correspond to a git repository.
    #[error("path is not a git repository")]
    NotARepository,
    /// Repository is bare and unsupported.
    #[error("repository is bare and unsupported")]
    BareRepository,
    /// Repository has no head revision to diff.
    #[error("repository has no head revision to diff")]
    MissingHeadRevision,
    /// Underlying git operation failed.
    #[error("git error")]
    Git,
    /// Filesystem interaction failed.
    #[error("filesystem error")]
    Io,
    /// Feature has not yet been implemented.
    #[error("feature not implemented")]
    Unimplemented,
    /// Internal invariant failed.
    #[error("internal error")]
    Internal,
}

impl From<Error> for CoreError {
    fn from(error: Error) -> Self {
        match error {
            Error::Unimplemented(_) => Self::Unimplemented,
            Error::Git { .. } => Self::Git,
            Error::NotARepository { .. } => Self::NotARepository,
            Error::BareRepository { .. } => Self::BareRepository,
            Error::Io { .. } => Self::Io,
            Error::MissingHeadRevision => Self::MissingHeadRevision,
        }
    }
}

impl<T> From<PoisonError<T>> for CoreError {
    fn from(_: PoisonError<T>) -> Self {
        Self::Internal
    }
}
