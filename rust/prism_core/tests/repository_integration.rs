use std::fs;
use std::path::Path;

use git2::{
    build::CheckoutBuilder, ErrorClass, ErrorCode, IndexAddOption, Repository as GitRepository,
};
use prism_core::repository::{Repository, RepositorySnapshot};
use prism_core::{Error, Result};
use tempfile::TempDir;

#[test]
fn snapshot_marks_workspace_dirty() -> Result<()> {
    let temp = TempDir::new().expect("tempdir");
    let git_repo = GitRepository::init(temp.path()).map_err(Error::from)?;

    write_file(temp.path().join("README.md"), "hello\n");
    commit_all(&git_repo, "initial")?;

    write_file(temp.path().join("README.md"), "hello world\n");

    let repo = Repository::open(temp.path())?;
    let snapshot = repo.snapshot()?;

    assert!(snapshot.workspace.dirty);
    assert!(snapshot.workspace.current_branch.is_some());

    Ok(())
}

#[test]
fn revision_range_returns_none_for_unborn_head() -> Result<()> {
    let temp = TempDir::new().expect("tempdir");
    GitRepository::init(temp.path()).map_err(Error::from)?;

    let repo = Repository::open(temp.path())?;
    let range = repo.revision_range()?;
    assert!(range.is_none());

    Ok(())
}

#[test]
fn revision_range_uses_first_parent_for_merge_commit() -> Result<()> {
    let temp = TempDir::new().expect("tempdir");
    let git_repo = GitRepository::init(temp.path()).map_err(Error::from)?;

    write_file(temp.path().join("file.txt"), "base\n");
    commit_all(&git_repo, "base")?;

    let main_branch = git_repo
        .head()
        .map_err(Error::from)?
        .shorthand()
        .unwrap_or("main")
        .to_string();
    let base_commit = git_repo
        .head()
        .map_err(Error::from)?
        .peel_to_commit()
        .map_err(Error::from)?;

    write_file(temp.path().join("file.txt"), "feature\n");
    commit_all(&git_repo, "feature commit")?;
    let feature_commit = git_repo
        .head()
        .map_err(Error::from)?
        .peel_to_commit()
        .map_err(Error::from)?;

    git_repo
        .branch("side", &base_commit, false)
        .map_err(Error::from)?;
    git_repo.set_head("refs/heads/side").map_err(Error::from)?;
    checkout_head_force(&git_repo)?;
    write_file(temp.path().join("file.txt"), "side\n");
    commit_all(&git_repo, "side commit")?;
    let side_commit = git_repo
        .head()
        .map_err(Error::from)?
        .peel_to_commit()
        .map_err(Error::from)?;

    let head_ref = format!("refs/heads/{main_branch}");
    git_repo.set_head(&head_ref).map_err(Error::from)?;
    checkout_head_force(&git_repo)?;
    write_file(temp.path().join("file.txt"), "merged\n");
    commit_with_parents(&git_repo, "merge commit", &[&feature_commit, &side_commit])?;

    let repo = Repository::open(temp.path())?;
    let range = repo.revision_range()?.expect("merge range");
    let base = range.base.expect("first parent present");

    assert_eq!(base.summary.as_deref(), Some("feature commit"));
    assert_eq!(base.oid, feature_commit.id().to_string());
    assert_eq!(range.head.summary.as_deref(), Some("merge commit"));

    Ok(())
}

#[test]
fn default_branch_falls_back_to_current_branch() -> Result<()> {
    let temp = TempDir::new().expect("tempdir");
    let git_repo = GitRepository::init(temp.path()).map_err(Error::from)?;

    write_file(temp.path().join("README.md"), "base\n");
    commit_all(&git_repo, "base")?;

    let repo = Repository::open(temp.path())?;
    let info = repo.info()?;
    let expected = git_repo
        .head()
        .map_err(Error::from)?
        .shorthand()
        .map(str::to_owned);

    assert_eq!(info.default_branch, expected);

    Ok(())
}

#[test]
fn default_branch_prefers_origin_head_when_present() -> Result<()> {
    let temp = TempDir::new().expect("tempdir");
    let git_repo = GitRepository::init(temp.path()).map_err(Error::from)?;

    write_file(temp.path().join("README.md"), "base\n");
    commit_all(&git_repo, "base")?;

    let head = git_repo.head().map_err(Error::from)?;
    let head_name = head
        .shorthand()
        .map(str::to_owned)
        .unwrap_or_else(|| "main".to_owned());
    let head_commit = head.peel_to_commit().map_err(Error::from)?;
    let head_oid = head_commit.id();

    let remote_branch = format!("refs/remotes/origin/{head_name}");
    git_repo
        .reference(&remote_branch, head_oid, true, "create origin branch")
        .map_err(Error::from)?;
    git_repo
        .reference_symbolic(
            "refs/remotes/origin/HEAD",
            &remote_branch,
            true,
            "point origin HEAD",
        )
        .map_err(Error::from)?;

    git_repo
        .branch("local-only", &head_commit, false)
        .map_err(Error::from)?;
    git_repo
        .set_head("refs/heads/local-only")
        .map_err(Error::from)?;
    checkout_head_force(&git_repo)?;

    let repo = Repository::open(temp.path())?;
    let info = repo.info()?;

    assert_eq!(info.default_branch.as_deref(), Some(head_name.as_str()));

    Ok(())
}

#[test]
fn repository_open_discovers_from_nested_path() -> Result<()> {
    let temp = TempDir::new().expect("tempdir");
    GitRepository::init(temp.path()).map_err(Error::from)?;
    let nested = temp.path().join("nested/deeper");
    fs::create_dir_all(&nested).expect("nested dirs");

    let repo = Repository::open(&nested)?;
    let repo_root = repo.root().canonicalize().expect("canonical root");
    let expected_root = temp.path().canonicalize().expect("canonical temp path");
    assert_eq!(repo_root, expected_root);

    Ok(())
}

#[test]
fn repository_open_rejects_bare_repository() {
    let temp = TempDir::new().expect("tempdir");
    let bare_path = temp.path().join("bare.git");
    GitRepository::init_bare(&bare_path).expect("bare repo");

    let err = Repository::open(&bare_path);
    assert!(matches!(err, Err(Error::BareRepository { .. })));
}

#[test]
fn snapshot_round_trips_through_serde() -> Result<()> {
    let temp = TempDir::new().expect("tempdir");
    let git_repo = GitRepository::init(temp.path()).map_err(Error::from)?;

    write_file(temp.path().join("README.md"), "# Prism\n");
    commit_all(&git_repo, "base")?;

    let repo = Repository::open(temp.path())?;
    let snapshot = repo.snapshot()?;

    let encoded = serde_json::to_string(&snapshot).expect("serialize snapshot");
    let decoded: RepositorySnapshot = serde_json::from_str(&encoded).expect("deserialize snapshot");

    assert_eq!(snapshot, decoded);

    Ok(())
}

fn commit_all(repo: &GitRepository, message: &str) -> Result<git2::Oid> {
    let parents = match repo.head() {
        Ok(reference) => {
            let commit = reference.peel_to_commit().map_err(Error::from)?;
            vec![commit]
        }
        Err(err)
            if matches!(
                (err.class(), err.code()),
                (
                    ErrorClass::Reference,
                    ErrorCode::NotFound | ErrorCode::UnbornBranch
                )
            ) =>
        {
            Vec::new()
        }
        Err(err) => return Err(Error::from(err)),
    };

    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    commit_with_parents(repo, message, parent_refs.as_slice())
}

fn commit_with_parents(
    repo: &GitRepository,
    message: &str,
    parents: &[&git2::Commit],
) -> Result<git2::Oid> {
    let mut index = repo.index().map_err(Error::from)?;
    index
        .add_all(["*"], IndexAddOption::DEFAULT, None)
        .map_err(Error::from)?;
    index.write().map_err(Error::from)?;
    let tree_id = index.write_tree().map_err(Error::from)?;
    let tree = repo.find_tree(tree_id).map_err(Error::from)?;
    let signature = git2::Signature::now("Test User", "test@example.com").map_err(Error::from)?;

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        parents,
    )
    .map_err(Error::from)
}

fn checkout_head_force(repo: &GitRepository) -> Result<()> {
    let mut checkout = CheckoutBuilder::new();
    checkout.force();
    repo.checkout_head(Some(&mut checkout)).map_err(Error::from)
}

fn write_file(path: impl AsRef<Path>, contents: &str) {
    fs::create_dir_all(
        path.as_ref()
            .parent()
            .expect("path should have a parent directory"),
    )
    .expect("create directories");
    fs::write(path, contents).expect("write file");
}
