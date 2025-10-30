//! Repository access and snapshot management built on top of libgit2.

use std::fmt;
use std::path::{Path, PathBuf};

use git2::{ErrorClass, ErrorCode, Repository as GitRepository, Status, StatusOptions};

use crate::{
    api::{RepositoryInfo, Revision, RevisionRange, Signature, WorkspaceStatus},
    Error, Result,
};

/// Immutable snapshot of the repository state that Prism uses as a baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySnapshot {
    /// Static metadata about the repository.
    pub info: RepositoryInfo,
    /// Current workspace status (branch/dirty).
    pub workspace: WorkspaceStatus,
    /// Revision range representing the pending review, if available.
    pub revisions: Option<RevisionRange>,
}

/// Lightweight handle to a repository that Prism operates on.
pub struct Repository {
    inner: GitRepository,
    root: PathBuf,
}

impl Repository {
    /// Open a repository from the given filesystem path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be canonicalized, does not resolve
    /// to a git repository, or if libgit2 reports an unsupported repository
    /// layout (such as a bare repository).
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let original = path.as_ref();
        let canonical = std::fs::canonicalize(original).map_err(|source| Error::Io {
            path: display_path(original),
            source,
        })?;

        let repo = match GitRepository::discover(&canonical) {
            Ok(repo) => repo,
            Err(err)
                if err.class() == ErrorClass::Repository && err.code() == ErrorCode::NotFound =>
            {
                return Err(Error::NotARepository {
                    path: display_path(&canonical),
                })
            }
            Err(err) => return Err(Error::from(err)),
        };

        let root = repo
            .workdir()
            .map(Path::to_path_buf)
            .ok_or_else(|| Error::BareRepository {
                path: display_path(&canonical),
            })?;

        Ok(Self { inner: repo, root })
    }

    /// Returns the absolute path to the repository root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns repository metadata.
    ///
    /// # Errors
    ///
    /// Propagates errors from querying the repository default branch.
    pub fn info(&self) -> Result<RepositoryInfo> {
        Ok(RepositoryInfo {
            root: display_path(&self.root),
            default_branch: self.default_branch()?,
        })
    }

    /// Returns the current workspace status.
    ///
    /// # Errors
    ///
    /// Propagates libgit2 status enumeration failures.
    pub fn workspace_status(&self) -> Result<WorkspaceStatus> {
        let current_branch = self.current_branch()?;
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true);

        let statuses = self.inner.statuses(Some(&mut opts))?;
        let dirty = statuses
            .iter()
            .any(|entry| entry.status() != Status::CURRENT);

        Ok(WorkspaceStatus {
            current_branch,
            dirty,
        })
    }

    /// Returns the current head revision, if the repository has one.
    ///
    /// # Errors
    ///
    /// Returns any error produced while resolving the HEAD reference.
    pub fn head_revision(&self) -> Result<Option<Revision>> {
        match self.head_commit()? {
            Some((reference, commit)) => Ok(Some(commit_to_revision(&commit, reference))),
            None => Ok(None),
        }
    }

    /// Returns the base revision (first parent of HEAD), if applicable.
    ///
    /// # Errors
    ///
    /// Returns any error produced while discovering the HEAD commit or its
    /// first parent.
    pub fn base_revision(&self) -> Result<Option<Revision>> {
        let Some((_, head)) = self.head_commit()? else {
            return Ok(None);
        };

        if head.parent_count() == 0 {
            return Ok(None);
        }

        let parent = head.parent(0)?;
        Ok(Some(commit_to_revision(&parent, None)))
    }

    /// Returns the revision range representing the workspace state.
    ///
    /// # Errors
    ///
    /// Returns any error produced while resolving the HEAD commit or its
    /// parent.
    pub fn revision_range(&self) -> Result<Option<RevisionRange>> {
        let Some((reference, head)) = self.head_commit()? else {
            return Ok(None);
        };

        let head_revision = commit_to_revision(&head, reference);
        let base_revision = if head.parent_count() == 0 {
            None
        } else {
            let parent = head.parent(0)?;
            Some(commit_to_revision(&parent, None))
        };

        Ok(Some(RevisionRange {
            base: base_revision,
            head: head_revision,
        }))
    }

    /// Captures a snapshot of the repository and workspace state.
    ///
    /// # Errors
    ///
    /// Returns an error if retrieving repository metadata, workspace status, or
    /// revision information fails.
    pub fn snapshot(&self) -> Result<RepositorySnapshot> {
        Ok(RepositorySnapshot {
            info: self.info()?,
            workspace: self.workspace_status()?,
            revisions: self.revision_range()?,
        })
    }

    fn default_branch(&self) -> Result<Option<String>> {
        let reference = match self.inner.find_reference("refs/remotes/origin/HEAD") {
            Ok(reference) => reference,
            Err(err) if err.code() == ErrorCode::NotFound => return Ok(None),
            Err(err) => return Err(Error::from(err)),
        };

        Ok(reference
            .symbolic_target()
            .and_then(|target| target.rsplit('/').next().map(str::to_owned)))
    }

    fn current_branch(&self) -> Result<Option<String>> {
        let head = match self.inner.head() {
            Ok(head) => head,
            Err(err)
                if matches!(
                    (err.class(), err.code()),
                    (
                        ErrorClass::Reference,
                        ErrorCode::NotFound | ErrorCode::UnbornBranch
                    )
                ) =>
            {
                return Ok(None)
            }
            Err(err) => return Err(Error::from(err)),
        };

        if head.is_branch() {
            Ok(head.shorthand().map(str::to_owned))
        } else {
            Ok(None)
        }
    }

    fn head_commit(&self) -> Result<Option<(Option<String>, git2::Commit<'_>)>> {
        let head = match self.inner.head() {
            Ok(head) => head,
            Err(err)
                if matches!(
                    (err.class(), err.code()),
                    (
                        ErrorClass::Reference,
                        ErrorCode::NotFound | ErrorCode::UnbornBranch
                    )
                ) =>
            {
                return Ok(None)
            }
            Err(err) => return Err(Error::from(err)),
        };

        let branch = if head.is_branch() {
            head.shorthand().map(str::to_owned)
        } else {
            None
        };

        let resolved = head.resolve()?;
        let commit = resolved.peel_to_commit()?;
        Ok(Some((branch, commit)))
    }
}

fn commit_to_revision(commit: &git2::Commit<'_>, reference: Option<String>) -> Revision {
    let author = commit.author();
    let committer = commit.committer();
    Revision {
        oid: commit.id().to_string(),
        reference,
        summary: commit.summary().map(str::to_owned),
        author: convert_signature(&author),
        committer: convert_signature(&committer),
        timestamp: Some(commit.time().seconds()),
    }
}

fn convert_signature(signature: &git2::Signature<'_>) -> Option<Signature> {
    signature.name().map(|name| Signature {
        name: name.to_owned(),
        email: signature.email().map(str::to_owned),
    })
}

fn display_path(path: &Path) -> String {
    path.to_path_buf()
        .into_os_string()
        .to_string_lossy()
        .into_owned()
}

impl fmt::Debug for Repository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Repository")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{IndexAddOption, Repository as GitRepository};
    use tempfile::TempDir;

    #[test]
    fn snapshot_reflects_clean_head() -> Result<()> {
        let temp = TempDir::new().expect("tempdir");
        let git_repo = GitRepository::init(temp.path())?;

        write_file(temp.path().join("README.md"), "hello\n");
        stage_and_commit(&git_repo, "Initial commit")?;

        let repo = Repository::open(temp.path())?;
        let snapshot = repo.snapshot()?;

        assert_eq!(snapshot.info.root, display_path(repo.root()));
        assert!(snapshot.workspace.current_branch.is_some());
        assert!(!snapshot.workspace.dirty);

        let revisions = snapshot.revisions.expect("head revision exists");
        assert!(revisions.base.is_none());
        assert_eq!(revisions.head.summary.as_deref(), Some("Initial commit"));

        Ok(())
    }

    #[test]
    fn base_revision_points_to_parent() -> Result<()> {
        let temp = TempDir::new().expect("tempdir");
        let git_repo = GitRepository::init(temp.path())?;

        write_file(temp.path().join("file.txt"), "one\n");
        stage_and_commit(&git_repo, "Initial commit")?;

        write_file(temp.path().join("file.txt"), "two\n");
        stage_and_commit(&git_repo, "Second commit")?;

        let repo = Repository::open(temp.path())?;
        let head = repo.head_revision()?.expect("head revision");
        let base = repo.base_revision()?.expect("base revision");

        assert_eq!(head.summary.as_deref(), Some("Second commit"));
        assert_eq!(base.summary.as_deref(), Some("Initial commit"));
        assert_ne!(head.oid, base.oid);

        Ok(())
    }

    #[test]
    fn open_non_repository_returns_error() {
        let temp = TempDir::new().expect("tempdir");
        let err = Repository::open(temp.path());
        assert!(matches!(err, Err(Error::NotARepository { .. })));
    }

    fn write_file(path: std::path::PathBuf, contents: &str) {
        std::fs::write(path, contents).expect("write file");
    }

    fn stage_and_commit(repo: &GitRepository, message: &str) -> Result<()> {
        let mut index = repo.index()?;
        index.add_all(["*"], IndexAddOption::DEFAULT, None)?;
        index.write()?;
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let signature = git2::Signature::now("Test User", "test@example.com")?;

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
