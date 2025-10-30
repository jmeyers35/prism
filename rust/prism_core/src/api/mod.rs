//! Public data models shared across Prism's FFI surface.
//!
//! The structures in this module are designed to be:
//! - serializable via `serde` for persistence and transport
//! - restricted to FFI-friendly primitives for future Swift bridging

/// Diff-related data types surfaced to the UI and plugins.
pub mod diff;
/// Repository metadata and revision representations.
pub mod repository;
/// Review comments, diagnostics, and suggestion models.
pub mod review;

pub use diff::{
    Diff, DiffFile, DiffHunk, DiffLine, DiffLineKind, DiffRange, DiffStats, FileStatus,
};
pub use repository::{RepositoryInfo, Revision, RevisionRange, Signature, WorkspaceStatus};
pub use review::{
    CommentDraft, Diagnostic, DiffSide, FileRange, Position, Range, ReviewComment, ReviewThread,
    Severity, Suggestion, TextEdit,
};
