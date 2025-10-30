//! Diff generation and patch application primitives.

use std::cell::RefCell;

use git2::{DiffFindOptions, DiffLineType, DiffOptions, Oid};

use crate::{
    api::diff::{
        Diff, DiffFile, DiffHunk, DiffLine, DiffLineKind, DiffRange, DiffStats, FileStatus,
    },
    repository::Repository,
    Error, Result,
};

/// Entry point for diff generation.
#[derive(Debug, Default)]
pub struct DiffEngine;

impl DiffEngine {
    /// Construct a new diff engine instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Generate a unified diff between the repository head and its base.
    ///
    /// # Errors
    ///
    /// Returns an error when the repository has no head revision or if any
    /// underlying git operation fails.
    pub fn diff(&self, repository: &Repository) -> Result<Diff> {
        let range = repository
            .revision_range()?
            .ok_or(Error::MissingHeadRevision)?;

        let git_repo = repository.git_repo();
        let head_tree = commit_tree(git_repo, &range.head.oid)?;
        let base_tree = match range.base.as_ref() {
            Some(base) => Some(commit_tree(git_repo, &base.oid)?),
            None => None,
        };

        let mut options = DiffOptions::new();
        options
            .context_lines(3)
            .interhunk_lines(0)
            .ignore_submodules(true)
            .indent_heuristic(true)
            .include_unmodified(true)
            .include_typechange(true);

        let mut raw_diff =
            git_repo.diff_tree_to_tree(base_tree.as_ref(), Some(&head_tree), Some(&mut options))?;

        let mut find_options = DiffFindOptions::new();
        find_options
            .renames(true)
            .renames_from_rewrites(true)
            .copies(true)
            .copies_from_unmodified(true)
            .copy_threshold(100)
            .break_rewrites_for_renames_only(true)
            .remove_unmodified(true);

        raw_diff.find_similar(Some(&mut find_options))?;

        let files = build_files(&raw_diff)?;

        Ok(Diff { range, files })
    }
}

fn commit_tree<'repo>(repo: &'repo git2::Repository, oid: &str) -> Result<git2::Tree<'repo>> {
    let oid = Oid::from_str(oid)?;
    let commit = repo.find_commit(oid)?;
    Ok(commit.tree()?)
}

fn build_files(diff: &git2::Diff<'_>) -> Result<Vec<DiffFile>> {
    let builder = RefCell::new(DiffBuilder::default());

    {
        let mut file_cb = |delta: git2::DiffDelta<'_>, _progress: f32| {
            builder.borrow_mut().start_file(&delta);
            true
        };

        let mut binary_cb = |_: git2::DiffDelta<'_>, _: git2::DiffBinary<'_>| {
            builder.borrow_mut().mark_binary();
            true
        };

        let mut hunk_cb = |_: git2::DiffDelta<'_>, hunk: git2::DiffHunk<'_>| {
            builder.borrow_mut().start_hunk(&hunk);
            true
        };

        let mut line_cb =
            |_: git2::DiffDelta<'_>, _: Option<git2::DiffHunk<'_>>, line: git2::DiffLine<'_>| {
                builder.borrow_mut().push_line(&line);
                true
            };

        diff.foreach(
            &mut file_cb,
            Some(&mut binary_cb),
            Some(&mut hunk_cb),
            Some(&mut line_cb),
        )?;
    }

    Ok(builder.into_inner().finish())
}

#[derive(Default)]
struct DiffBuilder {
    files: Vec<DiffFile>,
}

impl DiffBuilder {
    fn start_file(&mut self, delta: &git2::DiffDelta<'_>) {
        let status = convert_status(delta.status());
        let old_path = delta
            .old_file()
            .path()
            .map(|path| path.to_string_lossy().into_owned());
        let new_path = delta
            .new_file()
            .path()
            .map(|path| path.to_string_lossy().into_owned());

        let path = match (status, new_path.clone(), old_path.clone()) {
            (FileStatus::Deleted, _, Some(old)) | (_, None, Some(old)) => old,
            (_, Some(newer), _) => newer,
            _ => String::new(),
        };

        let diff_file = DiffFile {
            path,
            old_path: if matches!(status, FileStatus::Renamed | FileStatus::Copied)
                && old_path != new_path
            {
                old_path
            } else {
                None
            },
            status,
            stats: DiffStats::ZERO,
            is_binary: delta.new_file().is_binary() || delta.old_file().is_binary(),
            hunks: Vec::new(),
        };

        self.files.push(diff_file);
    }

    fn mark_binary(&mut self) {
        if let Some(current) = self.files.last_mut() {
            current.is_binary = true;
            current.hunks.clear();
        }
    }

    fn start_hunk(&mut self, hunk: &git2::DiffHunk<'_>) {
        if let Some(file) = self.files.last_mut() {
            if file.is_binary {
                return;
            }

            let header = DiffRange {
                base_start: hunk.old_start(),
                base_lines: hunk.old_lines(),
                head_start: hunk.new_start(),
                head_lines: hunk.new_lines(),
            };

            file.hunks.push(DiffHunk {
                header,
                section: parse_section(hunk.header()),
                lines: Vec::new(),
            });
        }
    }

    fn push_line(&mut self, line: &git2::DiffLine<'_>) {
        let Some(file) = self.files.last_mut() else {
            return;
        };

        if file.is_binary {
            return;
        }

        let Some(hunk) = file.hunks.last_mut() else {
            return;
        };

        let line_type = line.origin_value();
        let Some(kind) = convert_line_kind(line_type) else {
            return;
        };

        if matches!(kind, DiffLineKind::Addition) {
            file.stats.additions += 1;
        } else if matches!(kind, DiffLineKind::Deletion) {
            file.stats.deletions += 1;
        }

        hunk.lines.push(DiffLine {
            kind,
            text: sanitize_line(line.content()),
            base_line: line.old_lineno(),
            head_line: line.new_lineno(),
            highlights: Vec::new(),
        });
    }

    fn finish(self) -> Vec<DiffFile> {
        self.files
    }
}

const fn convert_status(status: git2::Delta) -> FileStatus {
    match status {
        git2::Delta::Added | git2::Delta::Untracked => FileStatus::Added,
        git2::Delta::Deleted => FileStatus::Deleted,
        git2::Delta::Modified
        | git2::Delta::Ignored
        | git2::Delta::Unreadable
        | git2::Delta::Conflicted
        | git2::Delta::Unmodified => FileStatus::Modified,
        git2::Delta::Renamed => FileStatus::Renamed,
        git2::Delta::Copied => FileStatus::Copied,
        git2::Delta::Typechange => FileStatus::TypeChange,
    }
}

fn parse_section(header: &[u8]) -> Option<String> {
    let header_str = std::str::from_utf8(header).ok()?;
    let trimmed = header_str.trim_end_matches(['\r', '\n']);
    let closing = trimmed.rfind("@@")?;
    let section = trimmed[closing + 2..].trim();
    if section.is_empty() {
        None
    } else {
        Some(section.to_owned())
    }
}

const fn convert_line_kind(origin: DiffLineType) -> Option<DiffLineKind> {
    match origin {
        DiffLineType::Context | DiffLineType::ContextEOFNL => Some(DiffLineKind::Context),
        DiffLineType::Addition | DiffLineType::AddEOFNL => Some(DiffLineKind::Addition),
        DiffLineType::Deletion | DiffLineType::DeleteEOFNL => Some(DiffLineKind::Deletion),
        DiffLineType::Binary | DiffLineType::FileHeader | DiffLineType::HunkHeader => None,
    }
}

fn sanitize_line(bytes: &[u8]) -> String {
    std::str::from_utf8(bytes).map_or_else(
        |_| {
            let text = String::from_utf8_lossy(bytes);
            trim_line_endings(text.to_string())
        },
        |text| trim_line_endings(text.to_owned()),
    )
}

fn trim_line_endings(mut line: String) -> String {
    if line.ends_with('\n') {
        line.pop();
        if line.ends_with('\r') {
            line.pop();
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{repository::Repository, Error};
    use git2::{IndexAddOption, Repository as GitRepository};
    use tempfile::TempDir;

    #[test]
    fn generates_diff_for_modified_file() -> Result<()> {
        let temp = TempDir::new().expect("tempdir");
        let git_repo = GitRepository::init(temp.path())?;

        write_file(temp.path().join("file.txt"), "hello\nworld\n");
        stage_and_commit(&git_repo, "Initial commit")?;

        write_file(temp.path().join("file.txt"), "hello\nprism\nworld\n");
        stage_and_commit(&git_repo, "Update greeting")?;

        let repository = Repository::open(temp.path())?;
        let engine = DiffEngine::new();
        let diff = engine.diff(&repository)?;

        assert_eq!(diff.files.len(), 1);
        let file = &diff.files[0];
        assert_eq!(file.path, "file.txt");
        assert_eq!(file.status, FileStatus::Modified);
        assert_eq!(file.stats.additions, 1);
        assert_eq!(file.stats.deletions, 0);
        assert_eq!(file.hunks.len(), 1);
        assert!(!file.hunks[0].lines.is_empty());

        let addition = file.hunks[0]
            .lines
            .iter()
            .find(|line| line.kind == DiffLineKind::Addition)
            .expect("addition line");
        assert_eq!(addition.head_line, Some(2));
        assert_eq!(addition.text, "prism");

        Ok(())
    }

    #[test]
    fn detects_renamed_file() -> Result<()> {
        let temp = TempDir::new().expect("tempdir");
        let git_repo = GitRepository::init(temp.path())?;

        write_file(temp.path().join("old.txt"), "one\n");
        stage_and_commit(&git_repo, "Initial commit")?;

        let old_path = temp.path().join("old.txt");
        let new_path = temp.path().join("new.txt");
        std::fs::rename(&old_path, &new_path).map_err(|source| Error::Io {
            path: old_path.to_string_lossy().into_owned(),
            source,
        })?;
        write_file(new_path, "one\ntwo\n");
        stage_and_commit(&git_repo, "Rename and edit")?;

        let repository = Repository::open(temp.path())?;
        let engine = DiffEngine::new();
        let diff = engine.diff(&repository)?;

        assert_eq!(diff.files.len(), 1);
        let file = &diff.files[0];
        assert_eq!(file.path, "new.txt");
        assert_eq!(file.old_path.as_deref(), Some("old.txt"));
        assert_eq!(file.status, FileStatus::Renamed);
        assert_eq!(file.stats.additions, 1);
        assert_eq!(file.stats.deletions, 0);

        Ok(())
    }

    #[test]
    fn handles_initial_commit_as_addition() -> Result<()> {
        let temp = TempDir::new().expect("tempdir");
        let git_repo = GitRepository::init(temp.path())?;

        write_file(temp.path().join("hello.txt"), "hi prism\n");
        stage_and_commit(&git_repo, "Initial commit")?;

        let repository = Repository::open(temp.path())?;
        let engine = DiffEngine::new();
        let diff = engine.diff(&repository)?;

        assert_eq!(diff.range.base, None);
        assert_eq!(diff.files.len(), 1);
        let file = &diff.files[0];
        assert_eq!(file.status, FileStatus::Added);
        assert_eq!(file.stats.additions, 1);
        assert_eq!(file.stats.deletions, 0);
        assert!(file.hunks.iter().all(|h| h.header.base_lines == 0));

        Ok(())
    }

    #[test]
    fn marks_deleted_file_with_deletions() -> Result<()> {
        let temp = TempDir::new().expect("tempdir");
        let git_repo = GitRepository::init(temp.path())?;

        write_file(temp.path().join("gone.txt"), "remove me\n");
        stage_and_commit(&git_repo, "Initial commit")?;

        std::fs::remove_file(temp.path().join("gone.txt")).expect("remove file");
        stage_and_commit(&git_repo, "Delete file")?;

        let repository = Repository::open(temp.path())?;
        let diff = DiffEngine::new().diff(&repository)?;

        assert_eq!(diff.files.len(), 1);
        let file = &diff.files[0];
        assert_eq!(file.path, "gone.txt");
        assert_eq!(file.status, FileStatus::Deleted);
        assert_eq!(file.stats.additions, 0);
        assert!(file.stats.deletions > 0);

        Ok(())
    }

    #[test]
    fn marks_binary_file_and_drops_hunks() -> Result<()> {
        let temp = TempDir::new().expect("tempdir");
        let git_repo = GitRepository::init(temp.path())?;

        write_bytes(temp.path().join("asset.bin"), &[0_u8, 159, 255, 0]);
        stage_and_commit(&git_repo, "Initial commit")?;

        write_bytes(temp.path().join("asset.bin"), &[5_u8, 6, 7, 8]);
        stage_and_commit(&git_repo, "Update binary")?;

        let repository = Repository::open(temp.path())?;
        let diff = DiffEngine::new().diff(&repository)?;

        assert_eq!(diff.files.len(), 1);
        let file = &diff.files[0];
        assert!(file.is_binary);
        assert!(file.hunks.is_empty());

        Ok(())
    }

    #[test]
    fn errors_when_repository_has_no_head() {
        let temp = TempDir::new().expect("tempdir");
        let git_repo = GitRepository::init(temp.path()).expect("init repo");

        let repository = Repository::open(temp.path()).expect("open repo");
        let result = DiffEngine::new().diff(&repository);

        drop(git_repo);

        assert!(matches!(result, Err(Error::MissingHeadRevision)));
    }

    #[test]
    fn parse_section_extracts_header_text() {
        let header = b"@@ -10,5 +10,6 @@ fn greeting";
        assert_eq!(
            parse_section(header).as_deref(),
            Some("fn greeting"),
            "expected trailing context to be parsed"
        );
    }

    #[test]
    fn parse_section_skips_empty_headers() {
        assert_eq!(parse_section(b"@@ -1,3 +1,4 @@\n"), None);
    }

    #[test]
    fn sanitize_line_handles_invalid_utf8() {
        let result = sanitize_line(&[b'f', b'o', 0xFF, b'o', b'\n']);
        assert_eq!(result, "fo\u{FFFD}o");
    }

    fn write_file(path: std::path::PathBuf, contents: &str) {
        std::fs::write(path, contents).expect("write file");
    }

    fn write_bytes(path: std::path::PathBuf, contents: &[u8]) {
        std::fs::write(path, contents).expect("write bytes");
    }

    fn stage_and_commit(repo: &GitRepository, message: &str) -> Result<()> {
        let mut index = repo.index()?;
        index.add_all(["*"], IndexAddOption::DEFAULT, None)?;
        index.write()?;
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let signature = git2::Signature::now("Test", "test@example.com")?;

        let parents = match repo.head() {
            Ok(head) => head
                .peel_to_commit()
                .map_or_else(|_| Vec::new(), |parent| vec![parent]),
            Err(err)
                if matches!(
                    (err.class(), err.code()),
                    (
                        git2::ErrorClass::Reference,
                        git2::ErrorCode::NotFound | git2::ErrorCode::UnbornBranch
                    )
                ) =>
            {
                Vec::new()
            }
            Err(err) => return Err(Error::from(err)),
        };

        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parent_refs,
        )?;
        Ok(())
    }
}
