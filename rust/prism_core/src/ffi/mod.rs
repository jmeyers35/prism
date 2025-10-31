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
        CommentDraft, Diagnostic, Diff, DiffFile, DiffHunk, DiffLine, DiffLineKind, DiffRange,
        DiffSide, DiffStats, FileRange, FileStatus, LineHighlight, PluginCapabilities,
        PluginSession, PluginSummary, Position, Range, RepositoryInfo, RepositorySnapshot,
        ReviewPayload, Revision, RevisionProgress, RevisionRange, RevisionState, Severity,
        Signature, SubmissionResult, Suggestion, TextEdit, ThreadRef, WorkspaceStatus,
    };

    uniffi::include_scaffolding!("prism_core");
}

pub use scaffolding::*;
