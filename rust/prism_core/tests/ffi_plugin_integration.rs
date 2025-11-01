use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use git2::Repository as GitRepository;
use prism_core::ffi::open;
use prism_core::{ReviewPayload, RevisionProgress, RevisionState};
use tempfile::TempDir;

static AMP_FFI_TEST_GUARD: Mutex<()> = Mutex::new(());

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
fn ffi_amp_plugin_invokes_cli_flows() {
    let _lock = AMP_FFI_TEST_GUARD.lock().expect("test guard");

    let temp = TempDir::new().expect("tempdir");
    GitRepository::init(temp.path()).expect("init repo");

    let amp_mock = AmpCliMock::new();

    let session = open(temp.path().to_string_lossy().into_owned()).expect("open session");

    let summaries = session.plugins();
    assert!(summaries.iter().any(|plugin| plugin.id == "amp"));

    let threads = session
        .plugin_threads("amp".into())
        .expect("list amp threads");
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].id, "T-sample");

    let plugin_session = session
        .attach_plugin("amp".into(), Some("T-sample".into()))
        .expect("attach amp");

    let payload = ReviewPayload {
        summary: Some("Ship revisions".into()),
        ..ReviewPayload::default()
    };
    session
        .post_review(plugin_session.clone(), payload)
        .expect("post review");

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let progress: RevisionProgress = session
            .poll_revision(plugin_session.clone())
            .expect("poll revision");
        if progress.state == RevisionState::Completed {
            break;
        }
        if Instant::now() > deadline {
            panic!("timed out waiting for amp revision");
        }
        thread::sleep(Duration::from_millis(25));
    }

    let captured = fs::read_to_string(amp_mock.capture_path()).expect("capture file");
    assert!(captured.contains("Ship revisions"));
}

struct AmpCliMock {
    _root: TempDir,
    capture: PathBuf,
}

impl AmpCliMock {
    fn new() -> Self {
        let root = TempDir::new().expect("temp dir");
        let script = root.path().join("amp_mock.sh");
        let capture = root.path().join("capture.txt");
        write_mock_script(&script, &capture);
        std::env::set_var("PRISM_AMP_CLI_BIN", &script);
        std::env::set_var("PRISM_AMP_CAPTURE", &capture);
        Self {
            _root: root,
            capture,
        }
    }

    fn capture_path(&self) -> &Path {
        &self.capture
    }
}

impl Drop for AmpCliMock {
    fn drop(&mut self) {
        std::env::remove_var("PRISM_AMP_CLI_BIN");
        std::env::remove_var("PRISM_AMP_CAPTURE");
    }
}

fn write_mock_script(script_path: &Path, capture_path: &Path) {
    let content = format!(
        r#"#!/bin/sh

set -eu

if [ "$1" = "threads" ] && [ "$2" = "list" ]; then
  cat <<'EOF'
Title                                         Last Updated  Visibility  Messages  Thread ID
────────────────────────────────────────────  ────────────  ──────────  ────────  ──────────────────────────────────────
Sample Thread                                 1d ago        Private            3  T-sample
EOF
  exit 0
fi

if [ "$1" = "threads" ] && [ "$2" = "new" ]; then
  echo "T-created"
  exit 0
fi

if [ "$1" = "threads" ] && [ "$2" = "continue" ]; then
  shift 3
  body=$(cat)
  printf "%s" "$body" > "{capture}"
  printf "Applied revisions\n"
  exit 0
fi

echo "Unsupported invocation: $@" >&2
exit 1
"#,
        capture = capture_path.display()
    );
    fs::write(script_path, content).expect("write amp script");
    let mut perms = fs::metadata(script_path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(script_path, perms).expect("set perms");
    fs::write(capture_path, "").expect("touch capture");
}
