use std::collections::HashMap;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::Path;

use git2::{IndexAddOption, Repository as GitRepository};
use prism_core::{api::diff::FileStatus, diff::DiffEngine, repository::Repository, Error, Result};
use tempfile::TempDir;

#[test]
fn diff_engine_reports_mixed_changes() -> Result<()> {
    let temp = TempDir::new().expect("tempdir");
    let git_repo = GitRepository::init(temp.path())?;

    write_text(temp.path().join("keep.txt"), "one\n");
    write_text(temp.path().join("remove.txt"), "bye\n");
    write_text(temp.path().join("rename.txt"), "original\n");
    stage_and_commit(&git_repo, "Initial commit")?;

    write_text(temp.path().join("keep.txt"), "one\nplus\n");
    write_text(temp.path().join("rename.txt"), "original\nupdate\n");
    write_text(temp.path().join("added.txt"), "brand new\n");
    rename_path(
        temp.path().join("rename.txt"),
        temp.path().join("renamed.txt"),
    )?;
    remove_path(temp.path().join("remove.txt"))?;
    stage_all(&git_repo, &["remove.txt", "rename.txt"])?;
    commit(&git_repo, "Mixed changes")?;

    let repository = Repository::open(temp.path())?;
    let diff = DiffEngine::new().diff(&repository)?;

    assert!(diff.range.base.is_some());
    assert_eq!(diff.files.len(), 4);

    let by_path: HashMap<_, _> = diff
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect();

    let keep = by_path.get("keep.txt").expect("keep diff");
    assert_eq!(keep.status, FileStatus::Modified);
    assert_eq!(keep.stats.additions, 1);

    let added = by_path.get("added.txt").expect("added diff");
    assert_eq!(added.status, FileStatus::Added);
    assert_eq!(added.stats.additions, 1);

    let removed = by_path.get("remove.txt").expect("removed diff");
    assert_eq!(removed.status, FileStatus::Deleted);
    assert!(removed.stats.deletions > 0);

    let renamed = by_path.get("renamed.txt").expect("renamed diff");
    assert_eq!(renamed.status, FileStatus::Renamed);
    assert_eq!(renamed.old_path.as_deref(), Some("rename.txt"));
    assert!(renamed.stats.additions > 0);

    Ok(())
}

#[test]
fn diff_engine_errors_without_head_commit() {
    let temp = TempDir::new().expect("tempdir");
    GitRepository::init(temp.path()).expect("init repo");

    let repository = Repository::open(temp.path()).expect("open repo");
    let result = DiffEngine::new().diff(&repository);
    assert!(matches!(result, Err(Error::MissingHeadRevision)));
}

#[test]
fn diff_engine_detects_copied_file() -> Result<()> {
    let temp = TempDir::new().expect("tempdir");
    let git_repo = GitRepository::init(temp.path())?;

    write_text(temp.path().join("original.txt"), "hello\n");
    stage_and_commit(&git_repo, "Initial commit")?;

    let original = temp.path().join("original.txt");
    let copy = temp.path().join("copy.txt");
    std::fs::copy(&original, &copy).expect("copy file");
    stage_and_commit(&git_repo, "Add copied file")?;

    let repository = Repository::open(temp.path())?;
    let diff = DiffEngine::new().diff(&repository)?;

    let copied = diff
        .files
        .iter()
        .find(|file| file.path == "copy.txt")
        .expect("copied diff");
    assert_eq!(copied.status, FileStatus::Copied);
    assert_eq!(copied.old_path.as_deref(), Some("original.txt"));

    Ok(())
}

#[cfg(unix)]
#[test]
fn diff_engine_flags_type_change() -> Result<()> {
    let temp = TempDir::new().expect("tempdir");
    let git_repo = GitRepository::init(temp.path())?;

    let path = temp.path().join("node");
    write_text(&path, "plain file\n");
    stage_and_commit(&git_repo, "Initial commit")?;

    std::fs::remove_file(&path).expect("remove plain file");
    symlink("target", &path).expect("create symlink");
    stage_and_commit(&git_repo, "Convert to symlink")?;

    let repository = Repository::open(temp.path())?;
    let diff = DiffEngine::new().diff(&repository)?;

    let entry = diff
        .files
        .iter()
        .find(|file| file.path == "node")
        .expect("typechange diff");
    assert_eq!(entry.status, FileStatus::TypeChange);

    Ok(())
}

#[cfg(not(unix))]
#[test]
fn diff_engine_flags_type_change() {
    // Symlink creation requires Unix capabilities; skip on other platforms.
    assert!(true);
}

fn write_text(path: impl AsRef<Path>, contents: &str) {
    std::fs::write(path, contents).expect("write text file");
}

fn rename_path(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
    let from_path = from.as_ref();
    let to_path = to.as_ref();
    std::fs::rename(from_path, to_path).map_err(|source| Error::Io {
        path: from_path.to_string_lossy().into_owned(),
        source,
    })?;
    Ok(())
}

fn remove_path(path: impl AsRef<Path>) -> Result<()> {
    let path_ref = path.as_ref();
    std::fs::remove_file(path_ref).map_err(|source| Error::Io {
        path: path_ref.to_string_lossy().into_owned(),
        source,
    })?;
    Ok(())
}

fn stage_and_commit(repo: &GitRepository, message: &str) -> Result<()> {
    stage_all(repo, &[])?;
    commit(repo, message)
}

fn stage_all(repo: &GitRepository, removals: &[&str]) -> Result<()> {
    let mut index = repo.index()?;
    index.add_all(["*"], IndexAddOption::DEFAULT, None)?;
    for path in removals {
        index.remove_path(Path::new(path))?;
    }
    index.write()?;
    Ok(())
}

fn commit(repo: &GitRepository, message: &str) -> Result<()> {
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
