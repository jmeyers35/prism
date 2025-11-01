//! Facilities for applying suggestion text edits to the workspace.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use git2::Patch;

use crate::api::{DiffSide, FileRange, Position, Suggestion, TextEdit};
use crate::repository::Repository;
use crate::Result;

/// Preview of applying a suggestion to a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyPreview {
    /// Relative path of the file the patch targets.
    pub path: String,
    /// Unified diff patch representing the changes.
    pub patch: String,
}

/// Applies suggestion text edits against a repository workspace.
pub struct SuggestionApplier<'repo> {
    repository: &'repo Repository,
}

impl<'repo> SuggestionApplier<'repo> {
    /// Construct a new applier bound to the provided repository.
    #[must_use]
    pub const fn new(repository: &'repo Repository) -> Self {
        Self { repository }
    }

    /// Compute the patches a suggestion would produce without mutating disk.
    ///
    /// # Errors
    ///
    /// Returns an error if any edit references an unsupported diff side, the
    /// target file cannot be read, or a range falls outside of the current
    /// workspace contents.
    pub fn dry_run(&self, suggestion: &Suggestion) -> Result<Vec<ApplyPreview>> {
        let changes = self.compute_changes(suggestion)?;
        let mut previews = Vec::with_capacity(changes.len());

        for change in changes {
            if change.original == change.updated {
                continue;
            }

            let patch =
                build_patch(&change.path, &change.original, &change.updated).map_err(|source| {
                    SuggestionError::GitDiff {
                        path: change.path.clone(),
                        source,
                    }
                })?;

            previews.push(ApplyPreview {
                path: change.path,
                patch,
            });
        }

        Ok(previews)
    }

    /// Apply the given suggestion to the working tree and update the index.
    ///
    /// # Errors
    ///
    /// Returns an error if any edit is invalid, if a file write fails, or if
    /// staging updated paths in the index cannot be completed.
    pub fn apply(&self, suggestion: &Suggestion) -> Result<()> {
        let changes = self.compute_changes(suggestion)?;
        if changes.is_empty() {
            return Ok(());
        }

        let repo_root = self.repository.root().to_path_buf();
        let git_repo = self.repository.git_repo();
        let mut index = git_repo.index()?;

        for change in &changes {
            if change.original == change.updated {
                continue;
            }

            let target_path = repo_root.join(&change.path);
            fs::write(&target_path, &change.updated).map_err(|source| SuggestionError::Io {
                path: change.path.clone(),
                source,
            })?;

            index.add_path(Path::new(&change.path)).map_err(|source| {
                SuggestionError::GitApply {
                    path: change.path.clone(),
                    source,
                }
            })?;
        }

        index.write()?;
        Ok(())
    }

    fn compute_changes(&self, suggestion: &Suggestion) -> Result<Vec<FileChange>> {
        let mut grouped: BTreeMap<&str, Vec<&TextEdit>> = BTreeMap::new();
        for edit in &suggestion.edits {
            grouped
                .entry(edit.location.path.as_str())
                .or_default()
                .push(edit);
        }

        let mut changes = Vec::with_capacity(grouped.len());
        for (path, edits) in grouped {
            let change = self.build_change(path, &edits)?;
            changes.push(change);
        }

        Ok(changes)
    }

    fn build_change(&self, path: &str, edits: &[&TextEdit]) -> Result<FileChange> {
        let repo_root = self.repository.root();
        let absolute = sanitize_path(repo_root, path)?;
        let original = fs::read_to_string(&absolute).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                SuggestionError::MissingFile {
                    path: path.to_owned(),
                }
            } else {
                SuggestionError::Io {
                    path: path.to_owned(),
                    source,
                }
            }
        })?;

        let converter = OffsetConverter::new(&original);
        let mut replacements = Vec::with_capacity(edits.len());

        for edit in edits {
            let (start, end) = converter
                .span(&edit.location)
                .map_err(|err| err.with_path(path))?;
            replacements.push(Replacement {
                start,
                end,
                replacement: edit.replacement.clone(),
            });
        }

        replacements.sort_by_key(|replacement| replacement.start);
        ensure_non_overlapping(path, &replacements)?;

        let mut updated = original.clone();
        for replacement in replacements.iter().rev() {
            updated.replace_range(replacement.start..replacement.end, &replacement.replacement);
        }

        Ok(FileChange {
            path: path.to_owned(),
            original,
            updated,
        })
    }
}

fn sanitize_path(root: &Path, relative: &str) -> std::result::Result<PathBuf, SuggestionError> {
    let candidate = Path::new(relative);
    if candidate.is_absolute() {
        return Err(SuggestionError::AbsolutePath {
            path: relative.to_owned(),
        });
    }

    if candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(SuggestionError::PathTraversal {
            path: relative.to_owned(),
        });
    }

    let normalized = root.join(candidate);
    Ok(normalized)
}

fn ensure_non_overlapping(
    path: &str,
    edits: &[Replacement],
) -> std::result::Result<(), SuggestionError> {
    for window in edits.windows(2) {
        let [first, second] = match window {
            [a, b] => [a, b],
            _ => continue,
        };

        if first.end > second.start {
            return Err(SuggestionError::OverlappingEdits {
                path: path.to_owned(),
            });
        }
    }

    Ok(())
}

fn build_patch(
    path: &str,
    original: &str,
    updated: &str,
) -> std::result::Result<String, git2::Error> {
    let path_ref = Path::new(path);
    let mut patch = Patch::from_buffers(
        original.as_bytes(),
        Some(path_ref),
        updated.as_bytes(),
        Some(path_ref),
        None,
    )?;

    let buffer = patch.to_buf()?;
    Ok(String::from_utf8_lossy(buffer.as_ref()).into_owned())
}

#[derive(Debug)]
struct FileChange {
    path: String,
    original: String,
    updated: String,
}

#[derive(Debug)]
struct Replacement {
    start: usize,
    end: usize,
    replacement: String,
}

#[derive(Debug)]
struct OffsetConverter<'a> {
    text: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> OffsetConverter<'a> {
    fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0];
        for (index, ch) in text.char_indices() {
            if ch == '\n' {
                line_starts.push(index + 1);
            }
        }

        Self { text, line_starts }
    }

    fn span(&self, range: &FileRange) -> std::result::Result<(usize, usize), SuggestionError> {
        if range.side != DiffSide::Head {
            return Err(SuggestionError::UnsupportedSide {
                path: range.path.clone(),
                side: range.side,
            });
        }

        let start = self.offset(&range.range.start)?;
        let end = self.offset(&range.range.end)?;

        if start > end {
            return Err(SuggestionError::InvalidRange {
                path: range.path.clone(),
                start,
                end,
            });
        }

        Ok((start, end))
    }

    fn offset(&self, position: &Position) -> std::result::Result<usize, SuggestionError> {
        if position.line == 0 {
            return Err(SuggestionError::LineOutOfBounds {
                path: String::new(),
                line: position.line,
            });
        }

        let line_index =
            usize::try_from(position.line - 1).map_err(|_| SuggestionError::LineOutOfBounds {
                path: String::new(),
                line: position.line,
            })?;
        if line_index > self.line_starts.len() {
            return Err(SuggestionError::LineOutOfBounds {
                path: String::new(),
                line: position.line,
            });
        }

        if line_index == self.line_starts.len() {
            if position.column.unwrap_or(1) == 1 {
                return Ok(self.text.len());
            }

            return Err(SuggestionError::ColumnOutOfBounds {
                path: String::new(),
                line: position.line,
                column: position.column.unwrap_or(0),
            });
        }

        let line_start = self.line_starts[line_index];
        let line_end = if line_index + 1 < self.line_starts.len() {
            self.line_starts[line_index + 1]
        } else {
            self.text.len()
        };

        let target_column = position.column.unwrap_or(1);
        if target_column == 1 {
            return Ok(line_start);
        }

        let slice = &self.text[line_start..line_end];
        let mut column = 1;
        for (offset, _) in slice.char_indices() {
            if column == target_column {
                return Ok(line_start + offset);
            }
            column += 1;
        }

        if column == target_column {
            return Ok(line_end);
        }

        Err(SuggestionError::ColumnOutOfBounds {
            path: String::new(),
            line: position.line,
            column: target_column,
        })
    }
}

/// Errors surfaced while translating or applying suggestions.
#[derive(Debug, thiserror::Error)]
pub enum SuggestionError {
    /// File path escaped the repository root.
    #[error("suggestion path must be relative: {path}")]
    AbsolutePath {
        /// Provided path.
        path: String,
    },
    /// File path attempted to traverse outside of the repository root.
    #[error("suggestion path must not contain parent segments: {path}")]
    PathTraversal {
        /// Provided path.
        path: String,
    },
    /// Edits must target the head side of the diff.
    #[error("suggestion edits for {path} must target the diff head, found {side:?}")]
    UnsupportedSide {
        /// Target file.
        path: String,
        /// Side referenced by the suggestion.
        side: DiffSide,
    },
    /// Referenced file is not present in the working tree.
    #[error("suggestion references missing file: {path}")]
    MissingFile {
        /// Missing file path.
        path: String,
    },
    /// Line index falls outside of the document bounds.
    #[error("line {line} is out of bounds for {path}")]
    LineOutOfBounds {
        /// Problematic file path.
        path: String,
        /// Requested line.
        line: u32,
    },
    /// Column index is invalid for the referenced line.
    #[error("column {column} on line {line} is out of bounds for {path}")]
    ColumnOutOfBounds {
        /// Problematic file path.
        path: String,
        /// Requested line.
        line: u32,
        /// Requested column.
        column: u32,
    },
    /// Multiple edits overlapped in the same file.
    #[error("suggestion edits overlap in {path}")]
    OverlappingEdits {
        /// File containing conflicting edits.
        path: String,
    },
    /// Range start exceeds range end.
    #[error("suggestion range is invalid in {path} (start {start} > end {end})")]
    InvalidRange {
        /// File containing the invalid range.
        path: String,
        /// Computed start offset.
        start: usize,
        /// Computed end offset.
        end: usize,
    },
    /// Failed while staging file updates in the index.
    #[error("failed to stage {path} in index: {source}")]
    GitApply {
        /// Target file path.
        path: String,
        /// Underlying git error.
        #[source]
        source: git2::Error,
    },
    /// Diff construction failed.
    #[error("failed to build diff for {path}: {source}")]
    GitDiff {
        /// Target file path.
        path: String,
        /// Underlying git error.
        #[source]
        source: git2::Error,
    },
    /// File write failed while applying edits.
    #[error("failed to write {path}: {source}")]
    Io {
        /// File targeted by the write.
        path: String,
        /// Source error.
        #[source]
        source: std::io::Error,
    },
}

impl SuggestionError {
    fn with_path(self, path: &str) -> Self {
        match self {
            Self::LineOutOfBounds { line, .. } => Self::LineOutOfBounds {
                path: path.to_owned(),
                line,
            },
            Self::ColumnOutOfBounds { line, column, .. } => Self::ColumnOutOfBounds {
                path: path.to_owned(),
                line,
                column,
            },
            other => other,
        }
    }
}
