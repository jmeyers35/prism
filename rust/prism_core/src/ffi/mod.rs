mod error;
mod session;

pub use error::CoreError;
pub use session::{open, CoreSession};

#[allow(
    clippy::doc_markdown,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::empty_line_after_doc_comments,
    clippy::missing_safety_doc
)]
mod scaffolding {
    use super::{open, CoreError, CoreSession};
    use crate::{
        Diff, DiffFile, DiffHunk, DiffLine, DiffLineKind, DiffRange, DiffStats, FileStatus,
        LineHighlight, RepositoryInfo, RepositorySnapshot, Revision, RevisionRange, Signature,
        WorkspaceStatus,
    };

    uniffi::include_scaffolding!("prism_core");
}

pub use scaffolding::*;
