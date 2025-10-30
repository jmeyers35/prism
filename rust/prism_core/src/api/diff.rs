use serde::{Deserialize, Serialize};

use super::repository::RevisionRange;

/// A full diff produced for a given revision range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diff {
    /// The revisions that were compared to produce this diff.
    pub range: RevisionRange,
    /// File-level diffs contained in this diff.
    #[serde(default)]
    pub files: Vec<DiffFile>,
}

/// Representation of the diff for a single file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffFile {
    /// Path of the file relative to the repository root.
    pub path: String,
    /// Previous path when the file was renamed or copied.
    #[serde(default)]
    pub old_path: Option<String>,
    /// Status of the file change in the diff.
    pub status: FileStatus,
    /// High-level summary of insertions/deletions.
    #[serde(default)]
    pub stats: DiffStats,
    /// Indicates whether the diff content is binary.
    #[serde(default)]
    pub is_binary: bool,
    /// The hunks that make up this file diff.
    #[serde(default)]
    pub hunks: Vec<DiffHunk>,
}

/// Summary information about the changes within a file diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DiffStats {
    /// Number of added lines.
    pub additions: u32,
    /// Number of removed lines.
    pub deletions: u32,
}

impl DiffStats {
    /// A stats instance with zero additions and deletions.
    pub const ZERO: Self = Self {
        additions: 0,
        deletions: 0,
    };

    /// Convenience constructor for explicit values.
    pub const fn new(additions: u32, deletions: u32) -> Self {
        Self {
            additions,
            deletions,
        }
    }

    /// Combine two stats structs.
    pub const fn add(self, other: Self) -> Self {
        Self {
            additions: self.additions + other.additions,
            deletions: self.deletions + other.deletions,
        }
    }
}

/// A diff hunk containing a contiguous set of changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffHunk {
    /// The range header describing the hunk offsets.
    pub header: DiffRange,
    /// Optional section header (e.g., function signature) extracted from the diff.
    #[serde(default)]
    pub section: Option<String>,
    /// Line-level changes inside the hunk.
    #[serde(default)]
    pub lines: Vec<DiffLine>,
}

/// The line number ranges referenced by a hunk header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffRange {
    /// Starting line number for the base side.
    pub base_start: u32,
    /// Number of lines covered on the base side.
    pub base_lines: u32,
    /// Starting line number for the head side.
    pub head_start: u32,
    /// Number of lines covered on the head side.
    pub head_lines: u32,
}

/// A single line within a diff hunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    /// The role the line plays in the diff (context, addition, deletion).
    pub kind: DiffLineKind,
    /// Raw text of the line.
    pub text: String,
    /// 1-based line number on the base side if applicable.
    #[serde(default)]
    pub base_line: Option<u32>,
    /// 1-based line number on the head side if applicable.
    #[serde(default)]
    pub head_line: Option<u32>,
    /// Optional inline highlights (e.g., intraline differences).
    #[serde(default)]
    pub highlights: Vec<LineHighlight>,
}

/// Highlights to indicate intraline modifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineHighlight {
    /// Zero-based column where the highlight begins (inclusive).
    pub start_column: u32,
    /// Zero-based column where the highlight ends (exclusive).
    pub end_column: u32,
}

/// Type of a line contained in a diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffLineKind {
    /// Unchanged context line.
    Context,
    /// A newly added line.
    Addition,
    /// A deleted line.
    Deletion,
}

/// File status from the diff's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    /// File only exists in the head side.
    Added,
    /// File only exists in the base side.
    Deleted,
    /// File exists on both sides with modifications.
    Modified,
    /// File path changed between base and head.
    Renamed,
    /// File content copied from another location.
    Copied,
    /// File type changed (e.g., text -> binary).
    TypeChange,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::repository::{Revision, RevisionRange, Signature};

    #[test]
    fn diff_round_trip() {
        let diff = Diff {
            range: RevisionRange {
                base: Some(Revision {
                    oid: "1111111111111111111111111111111111111111".into(),
                    reference: Some("main".into()),
                    summary: Some("Base commit".into()),
                    author: Some(Signature {
                        name: "Base Author".into(),
                        email: Some("base@example.com".into()),
                    }),
                    committer: None,
                    timestamp: Some(1_700_000_000),
                }),
                head: Revision {
                    oid: "2222222222222222222222222222222222222222".into(),
                    reference: Some("feature".into()),
                    summary: Some("Head commit".into()),
                    author: Some(Signature {
                        name: "Head Author".into(),
                        email: Some("head@example.com".into()),
                    }),
                    committer: None,
                    timestamp: Some(1_700_000_100),
                },
            },
            files: vec![DiffFile {
                path: "src/lib.rs".into(),
                old_path: None,
                status: FileStatus::Modified,
                stats: DiffStats::new(2, 1),
                is_binary: false,
                hunks: vec![DiffHunk {
                    header: DiffRange {
                        base_start: 10,
                        base_lines: 5,
                        head_start: 10,
                        head_lines: 6,
                    },
                    section: Some("fn diff_round_trip".into()),
                    lines: vec![
                        DiffLine {
                            kind: DiffLineKind::Context,
                            text: "fn example() {".into(),
                            base_line: Some(10),
                            head_line: Some(10),
                            highlights: vec![],
                        },
                        DiffLine {
                            kind: DiffLineKind::Deletion,
                            text: "    println!(\"old\");".into(),
                            base_line: Some(11),
                            head_line: None,
                            highlights: vec![],
                        },
                        DiffLine {
                            kind: DiffLineKind::Addition,
                            text: "    println!(\"new\");".into(),
                            base_line: None,
                            head_line: Some(11),
                            highlights: vec![LineHighlight {
                                start_column: 15,
                                end_column: 18,
                            }],
                        },
                    ],
                }],
            }],
        };

        let json = serde_json::to_string_pretty(&diff).expect("serialize diff");
        let decoded: Diff = serde_json::from_str(&json).expect("deserialize diff");
        assert_eq!(diff, decoded);
    }

    #[test]
    fn diff_stats_add() {
        let aggregate = DiffStats::new(5, 3).add(DiffStats::new(2, 4));
        assert_eq!(
            aggregate,
            DiffStats {
                additions: 7,
                deletions: 7
            }
        );
    }

    #[test]
    fn serde_defaults_are_applied() {
        let json = r#"{
            "range": {
                "base": null,
                "head": {
                    "oid": "3333333333333333333333333333333333333333",
                    "reference": null,
                    "summary": null,
                    "author": null,
                    "committer": null,
                    "timestamp": null
                }
            },
            "files": [{
                "path": "README.md",
                "status": "added"
            }]
        }"#;

        let diff: Diff = serde_json::from_str(json).expect("deserialize with defaults");
        assert_eq!(diff.files.len(), 1);
        let file = &diff.files[0];
        assert_eq!(file.path, "README.md");
        assert_eq!(file.status, FileStatus::Added);
        assert_eq!(file.stats, DiffStats::ZERO);
        assert!(!file.is_binary);
        assert!(file.hunks.is_empty());
    }

    #[test]
    fn encoded_uses_snake_case() {
        let json = serde_json::to_string(&FileStatus::TypeChange).expect("serialize status");
        assert_eq!(json, "\"type_change\"");
        let status: FileStatus = serde_json::from_str(&json).expect("deserialize status");
        assert_eq!(status, FileStatus::TypeChange);

        let json = serde_json::to_string(&DiffLineKind::Addition).expect("serialize line kind");
        assert_eq!(json, "\"addition\"");
    }

    #[test]
    fn diff_range_is_copy() {
        let range = DiffRange {
            base_start: 1,
            base_lines: 2,
            head_start: 3,
            head_lines: 4,
        };
        let copied = range;
        assert_eq!(copied.base_start, 1);
    }

    #[test]
    fn line_highlight_bounds() {
        let highlight = LineHighlight {
            start_column: 0,
            end_column: 5,
        };
        assert!(highlight.start_column < highlight.end_column);
    }
}
