use serde::{Deserialize, Serialize};

use super::diff::DiffLineKind;

/// Side of the diff a location refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffSide {
    /// Base (often "left") side of the diff.
    Base,
    /// Head (often "right") side of the diff.
    Head,
}

impl DiffSide {
    /// Map a `DiffLineKind` to the side it primarily touches.
    pub const fn from_line_kind(kind: DiffLineKind) -> Self {
        match kind {
            DiffLineKind::Context | DiffLineKind::Deletion => DiffSide::Base,
            DiffLineKind::Addition => DiffSide::Head,
        }
    }
}

/// A 1-based line/column pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// 1-based line number.
    pub line: u32,
    /// Optional 1-based column.
    #[serde(default)]
    pub column: Option<u32>,
}

impl Position {
    /// Convenience constructor.
    pub const fn new(line: u32, column: Option<u32>) -> Self {
        Self { line, column }
    }
}

/// A range bounded by two positions (inclusive start, exclusive end).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    /// Inclusive start position.
    pub start: Position,
    /// Exclusive end position.
    pub end: Position,
}

impl Range {
    /// Construct a range with explicit start and end.
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}

/// A range within a single file on a specific diff side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRange {
    /// Path relative to the repository root.
    pub path: String,
    /// Which side of the diff the range targets.
    pub side: DiffSide,
    /// Line/column span.
    pub range: Range,
}

impl FileRange {
    /// Create a new file range.
    pub fn new(path: impl Into<String>, side: DiffSide, range: Range) -> Self {
        Self {
            path: path.into(),
            side,
            range,
        }
    }
}

/// A text edit used for suggestions or quick fixes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    /// File location affected by the edit.
    pub location: FileRange,
    /// Replacement text to apply.
    pub replacement: String,
}

impl TextEdit {
    /// Create a new text edit.
    pub fn new(location: FileRange, replacement: impl Into<String>) -> Self {
        Self {
            location,
            replacement: replacement.into(),
        }
    }
}

/// Suggested change associated with a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    /// Human-friendly title presented to the user.
    #[serde(default)]
    pub title: Option<String>,
    /// Individual text edits contained in the suggestion.
    #[serde(default)]
    pub edits: Vec<TextEdit>,
}

impl Suggestion {
    /// Create an empty suggestion.
    pub fn new(title: Option<impl Into<String>>) -> Self {
        Self {
            title: title.map(|t| t.into()),
            edits: Vec::new(),
        }
    }
}

/// Severity levels for plugin diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational context without required action.
    Info,
    /// Potential issue that may require attention.
    Warning,
    /// Definite issue that must be addressed before submission.
    Error,
}

/// Diagnostic emitted by an agent plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Short title summarizing the issue.
    pub title: String,
    /// Detailed explanation, if provided.
    #[serde(default)]
    pub detail: Option<String>,
    /// Severity level of the diagnostic.
    pub severity: Severity,
    /// Where the diagnostic applies.
    pub location: FileRange,
    /// Additional tags or categories attached by the plugin.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Suggested remediations.
    #[serde(default)]
    pub suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    /// Convenience constructor for diagnostics without suggestions.
    pub fn new(title: impl Into<String>, severity: Severity, location: FileRange) -> Self {
        Self {
            title: title.into(),
            detail: None,
            severity,
            location,
            tags: Vec::new(),
            suggestions: Vec::new(),
        }
    }
}

/// Draft comment prepared for submission by the UI or plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentDraft {
    /// Markdown-formatted body of the comment.
    pub body: String,
    /// Location the comment is anchored to.
    pub location: FileRange,
}

impl CommentDraft {
    /// Create a new comment draft.
    pub fn new(body: impl Into<String>, location: FileRange) -> Self {
        Self {
            body: body.into(),
            location,
        }
    }
}

/// A comment already published in a review thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewComment {
    /// Optional author display name.
    #[serde(default)]
    pub author: Option<String>,
    /// Markdown body of the comment.
    pub body: String,
    /// Unix timestamp (seconds) when the comment was created.
    #[serde(default)]
    pub created_at: Option<i64>,
}

/// An entire comment thread tied to a diff location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewThread {
    /// Optional unique identifier for the thread.
    #[serde(default)]
    pub id: Option<String>,
    /// Shared location for all comments in the thread.
    pub location: FileRange,
    /// Ordered comments contained in the thread.
    #[serde(default)]
    pub comments: Vec<ReviewComment>,
    /// Whether the thread has been marked as resolved.
    #[serde(default)]
    pub resolved: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_range(side: DiffSide) -> FileRange {
        FileRange::new(
            "src/lib.rs",
            side,
            Range::new(Position::new(10, Some(5)), Position::new(10, Some(15))),
        )
    }

    #[test]
    fn diagnostic_round_trip() {
        let mut suggestion = Suggestion::new(Some("Apply fix"));
        suggestion.edits.push(TextEdit::new(
            sample_range(DiffSide::Head),
            "println!(\"hello\");",
        ));

        let diagnostic = Diagnostic {
            title: "Remove unused variable".into(),
            detail: Some("The variable `x` is never read.".into()),
            severity: Severity::Warning,
            location: sample_range(DiffSide::Head),
            tags: vec!["lint".into(), "unused".into()],
            suggestions: vec![suggestion],
        };

        let json = serde_json::to_string_pretty(&diagnostic).expect("serialize diagnostic");
        let decoded: Diagnostic = serde_json::from_str(&json).expect("deserialize diagnostic");
        assert_eq!(diagnostic, decoded);
    }

    #[test]
    fn comment_thread_defaults() {
        let json = r#"{
            "location": {
                "path": "README.md",
                "side": "head",
                "range": {
                    "start": {"line": 5},
                    "end": {"line": 5}
                }
            }
        }"#;

        let thread: ReviewThread = serde_json::from_str(json).expect("deserialize comment thread");
        assert!(thread.id.is_none());
        assert!(thread.comments.is_empty());
        assert!(!thread.resolved);
        assert_eq!(thread.location.path, "README.md");
        assert_eq!(thread.location.side, DiffSide::Head);
        assert_eq!(thread.location.range.start.line, 5);
        assert!(thread.location.range.start.column.is_none());
    }

    #[test]
    fn diff_side_mapping() {
        assert_eq!(
            DiffSide::from_line_kind(DiffLineKind::Addition),
            DiffSide::Head
        );
        assert_eq!(
            DiffSide::from_line_kind(DiffLineKind::Deletion),
            DiffSide::Base
        );
        assert_eq!(
            DiffSide::from_line_kind(DiffLineKind::Context),
            DiffSide::Base
        );
    }

    #[test]
    fn range_constructor() {
        let range = Range::new(Position::new(1, None), Position::new(1, Some(1)));
        assert_eq!(range.start.line, 1);
        assert_eq!(range.end.column, Some(1));
    }
}
