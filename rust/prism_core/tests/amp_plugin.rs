use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use prism_core::plugins::{AgentPlugin, AmpPlugin, ReviewPayload, RevisionState};
use tempfile::TempDir;

static TEST_GUARD: Mutex<()> = Mutex::new(());

#[test]
fn amp_plugin_lists_threads() {
    let _lock = TEST_GUARD.lock().expect("test guard");
    let _mock = MockAmp::new(MockMode::Success);

    let plugin = AmpPlugin::default();
    let threads = plugin.list_threads().expect("list threads");

    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].id, "T-sample");
    assert_eq!(threads[0].title.as_deref(), Some("Sample Thread"));
}

#[test]
fn amp_plugin_creates_thread_when_none_provided() {
    let _lock = TEST_GUARD.lock().expect("test guard");
    let _mock = MockAmp::new(MockMode::Success);

    let plugin = AmpPlugin::default();
    let session = plugin.attach(None).expect("attach new thread");

    assert_eq!(
        session.thread.as_ref().map(|t| t.id.as_str()),
        Some("T-created"),
    );
}

#[test]
fn amp_plugin_posts_review_and_reports_completion() {
    let _lock = TEST_GUARD.lock().expect("test guard");
    let mock = MockAmp::new(MockMode::Success);

    let plugin = AmpPlugin::default();
    let session = plugin
        .attach(Some("T-sample"))
        .expect("attach existing thread");

    let payload = ReviewPayload {
        summary: Some("Please address the diff".into()),
        ..ReviewPayload::default()
    };

    plugin.post_review(&session, payload).expect("post review");

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let progress = plugin.poll_revision(&session).expect("poll revision");
        if progress.state == RevisionState::Completed {
            break;
        }
        if Instant::now() > deadline {
            panic!("timed out waiting for revision completion");
        }
        thread::sleep(Duration::from_millis(25));
    }

    let captured = fs::read_to_string(mock.capture_path()).expect("capture file");
    assert!(captured.contains("Please address the diff"));
}

#[test]
fn amp_plugin_reports_failed_post_review() {
    let _lock = TEST_GUARD.lock().expect("test guard");
    let mock = MockAmp::new(MockMode::ContinueFails);

    let plugin = AmpPlugin::default();
    let session = plugin
        .attach(Some("T-sample"))
        .expect("attach existing thread");

    let payload = ReviewPayload {
        summary: Some("Failure case".into()),
        ..ReviewPayload::default()
    };

    plugin.post_review(&session, payload).expect("post review");

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let progress = plugin.poll_revision(&session).expect("poll revision");
        if progress.state == RevisionState::Failed {
            let detail = progress.detail.expect("detail present");
            assert!(
                detail.contains("Amp CLI failed with status 1"),
                "unexpected detail: {detail}",
            );
            break;
        }
        if Instant::now() > deadline {
            panic!("timed out waiting for failure state");
        }
        thread::sleep(Duration::from_millis(25));
    }

    let captured = fs::read_to_string(mock.capture_path()).expect("capture file");
    assert!(captured.contains("Failure case"));
}

struct MockAmp {
    _root: TempDir,
    capture: PathBuf,
}

enum MockMode {
    Success,
    ContinueFails,
}

impl MockAmp {
    fn new(mode: MockMode) -> Self {
        let root = TempDir::new().expect("temp dir");
        let script = root.path().join("amp_mock.sh");
        let capture = root.path().join("capture.txt");
        write_script(&script, &capture, &mode);
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

impl Drop for MockAmp {
    fn drop(&mut self) {
        std::env::remove_var("PRISM_AMP_CLI_BIN");
        std::env::remove_var("PRISM_AMP_CAPTURE");
    }
}

fn write_script(script_path: &Path, capture_path: &Path, mode: &MockMode) {
    let continue_block = match mode {
        MockMode::Success => format!(
            r#"if [ "$1" = "threads" ] && [ "$2" = "continue" ]; then
  shift 3
  body=$(cat)
  printf "%s" "$body" > "{capture}"
  printf "Applied revisions\n"
  exit 0
fi"#,
            capture = capture_path.display()
        ),
        MockMode::ContinueFails => format!(
            r#"if [ "$1" = "threads" ] && [ "$2" = "continue" ]; then
  shift 3
  body=$(cat)
  printf "%s" "$body" > "{capture}"
  echo "boom" >&2
  exit 1
fi"#,
            capture = capture_path.display()
        ),
    };

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

{continue_block}

echo "Unsupported invocation: $@" >&2
exit 1
"#,
        continue_block = continue_block
    );
    fs::write(script_path, content).expect("write mock script");
    let mut perms = fs::metadata(script_path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(script_path, perms).expect("set perms");
    fs::write(capture_path, "").expect("init capture");
}
