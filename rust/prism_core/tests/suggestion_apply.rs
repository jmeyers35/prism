use std::path::{Path, PathBuf};

use git2::{IndexAddOption, Repository as GitRepository};
use prism_core::repository::Repository;
use prism_core::{
    DiffSide, Error, FileRange, Position, Range, Suggestion, SuggestionApplier, SuggestionError,
    TextEdit,
};
use tempfile::TempDir;

#[test]
fn dry_run_produces_patch_without_mutating_disk() -> prism_core::Result<()> {
    let fixture = RepoFixture::new()?;

    let repository = Repository::open(fixture.root())?;
    let applier = SuggestionApplier::new(&repository);

    let suggestion = line_replacement_suggestion();
    let previews = applier.dry_run(&suggestion)?;

    assert_eq!(previews.len(), 1);
    let preview = &previews[0];
    assert_eq!(preview.path, "file.txt");
    assert!(preview.patch.contains("-line 2"));
    assert!(preview.patch.contains("+line two"));

    let current = std::fs::read_to_string(fixture.path("file.txt")).expect("read file");
    assert_eq!(current, "line 1\nline 2\n");

    Ok(())
}

#[test]
fn apply_updates_working_tree_and_index() -> prism_core::Result<()> {
    let fixture = RepoFixture::new()?;

    let repository = Repository::open(fixture.root())?;
    let applier = SuggestionApplier::new(&repository);

    let suggestion = line_replacement_suggestion();
    applier.apply(&suggestion)?;

    let updated = std::fs::read_to_string(fixture.path("file.txt")).expect("read file");
    assert_eq!(updated, "line 1\nline two\n");

    let git_repo = fixture.git_repository();
    let mut index = git_repo.index()?;
    index.read(true)?;
    let entry = index
        .get_path(Path::new("file.txt"), 0)
        .expect("entry staged");
    let blob = git_repo.find_blob(entry.id)?;
    let staged = std::str::from_utf8(blob.content()).expect("utf8");
    assert_eq!(staged, "line 1\nline two\n");

    Ok(())
}

#[test]
fn apply_fails_when_range_out_of_bounds_and_preserves_file() -> prism_core::Result<()> {
    let fixture = RepoFixture::new()?;

    let repository = Repository::open(fixture.root())?;
    let applier = SuggestionApplier::new(&repository);

    // Mutate the working copy so the second line no longer exists.
    std::fs::write(fixture.path("file.txt"), "line 1").expect("write file");

    let suggestion = line_replacement_suggestion();
    let err = applier.apply(&suggestion).expect_err("apply should fail");

    match err {
        Error::Suggestion { source } => match source {
            SuggestionError::LineOutOfBounds { .. } => {}
            other => panic!("unexpected suggestion error: {other:?}"),
        },
        other => panic!("unexpected error: {other:?}"),
    }

    let persisted = std::fs::read_to_string(fixture.path("file.txt")).expect("read file");
    assert_eq!(persisted, "line 1");

    Ok(())
}

struct RepoFixture {
    dir: TempDir,
    git_repo: GitRepository,
}

impl RepoFixture {
    fn new() -> prism_core::Result<Self> {
        let dir = TempDir::new().expect("tempdir");
        let git_repo = GitRepository::init(dir.path())?;

        write_file(dir.path().join("file.txt"), "line 1\nline 2\n");
        stage_and_commit(&git_repo, "Initial commit")?;

        Ok(Self { dir, git_repo })
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }

    fn git_repository(&self) -> &GitRepository {
        &self.git_repo
    }
}

impl Drop for RepoFixture {
    fn drop(&mut self) {
        // Ensure index updates are flushed for subsequent inspections.
        if let Ok(mut index) = self.git_repo.index() {
            let _ = index.write();
        }
    }
}

fn line_replacement_suggestion() -> Suggestion {
    let mut suggestion = Suggestion::new(Some("Replace line"));
    suggestion.edits.push(TextEdit::new(
        FileRange::new(
            "file.txt",
            DiffSide::Head,
            Range::new(Position::new(2, Some(1)), Position::new(3, Some(1))),
        ),
        "line two\n",
    ));
    suggestion
}

fn write_file(path: PathBuf, contents: &str) {
    std::fs::write(path, contents).expect("write file");
}

fn stage_and_commit(repo: &GitRepository, message: &str) -> prism_core::Result<()> {
    let mut index = repo.index()?;
    index.add_all(["*"], IndexAddOption::DEFAULT, None)?;
    index.write()?;

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let signature = git2::Signature::now("Test User", "test@example.com")?;

    let parents = match repo.head() {
        Ok(head) => head
            .peel_to_commit()
            .map_or_else(|_| Vec::new(), |commit| vec![commit]),
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
