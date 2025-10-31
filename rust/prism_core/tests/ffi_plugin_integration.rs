use git2::Repository as GitRepository;
use prism_core::ffi::{open, CoreError};
use prism_core::{ReviewPayload, RevisionProgress, RevisionState};
use tempfile::TempDir;

#[test]
fn ffi_git_only_plugin_flow() {
    let temp = TempDir::new().expect("tempdir");
    GitRepository::init(temp.path()).expect("init repo");

    let session = open(temp.path().to_string_lossy().into_owned()).expect("open session");

    let summaries = session.plugins();
    assert!(summaries.iter().any(|plugin| plugin.id == "git-only"));

    let threads = session
        .plugin_threads("git-only".into())
        .expect("list git-only threads");
    assert!(threads.is_empty());

    let plugin_session = session
        .attach_plugin("git-only".into(), None)
        .expect("attach git-only");
    assert_eq!(plugin_session.plugin_id, "git-only");

    let payload = ReviewPayload {
        summary: Some("Looks good".into()),
        ..ReviewPayload::default()
    };
    let submission = session
        .post_review(plugin_session.clone(), payload)
        .expect("post review");
    assert!(!submission.revision_started);

    let progress: RevisionProgress = session
        .poll_revision(plugin_session)
        .expect("poll revision");
    assert_eq!(progress.state, RevisionState::Completed);
}

#[test]
fn ffi_amp_plugin_reports_unsupported_operations() {
    let temp = TempDir::new().expect("tempdir");
    GitRepository::init(temp.path()).expect("init repo");

    let session = open(temp.path().to_string_lossy().into_owned()).expect("open session");

    let summaries = session.plugins();
    assert!(summaries.iter().any(|plugin| plugin.id == "amp"));

    match session.plugin_threads("amp".into()) {
        Err(CoreError::Plugin) => {}
        other => panic!("expected plugin error, got {other:?}"),
    }

    match session.attach_plugin("amp".into(), Some("thread-123".into())) {
        Err(CoreError::Plugin) => {}
        other => panic!("expected plugin error on attach, got {other:?}"),
    }
}
