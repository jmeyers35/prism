use git2::Repository as GitRepository;
use prism_core::ffi::open;
use prism_core::plugins::{
    register_test_plugin, AgentPlugin, PluginCapabilities, PluginError, PluginSession,
    RevisionProgress, RevisionState, SubmissionResult, ThreadRef,
};
use prism_core::ReviewPayload;
use tempfile::TempDir;

#[derive(Debug, Default)]
struct FfiTestPlugin;

impl AgentPlugin for FfiTestPlugin {
    fn id(&self) -> &'static str {
        "ffi-test"
    }

    fn label(&self) -> &'static str {
        "FFI Test Plugin"
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities::new(true, true, true)
    }

    fn list_threads(&self) -> Result<Vec<ThreadRef>, PluginError> {
        Ok(vec![ThreadRef::new("ffi-thread", Some("FFI Thread"))])
    }

    fn attach(&self, thread_id: Option<&str>) -> Result<PluginSession, PluginError> {
        let thread_id = thread_id.unwrap_or("ffi-thread");
        let thread = Some(ThreadRef::new(thread_id, Some("FFI Thread")));
        let session_id = format!("ffi-session-{thread_id}");
        Ok(PluginSession::new(self.id(), session_id, thread))
    }

    fn post_review(
        &self,
        _session: &PluginSession,
        payload: ReviewPayload,
    ) -> Result<SubmissionResult, PluginError> {
        Ok(SubmissionResult {
            revision_started: true,
            reference: Some("ffi-ref".into()),
            message: payload.summary,
        })
    }

    fn poll_revision(&self, _session: &PluginSession) -> Result<RevisionProgress, PluginError> {
        Ok(RevisionProgress {
            state: RevisionState::Completed,
            detail: Some("done".into()),
        })
    }
}

#[test]
fn ffi_fake_plugin_end_to_end() {
    let temp = TempDir::new().expect("tempdir");
    GitRepository::init(temp.path()).expect("init repo");

    let _guard = register_test_plugin(FfiTestPlugin);

    let session = open(temp.path().to_string_lossy().into_owned()).expect("open session");

    let summaries = session.plugins();
    assert!(summaries.iter().any(|plugin| plugin.id == "ffi-test"));

    let threads = session
        .plugin_threads("ffi-test".into())
        .expect("list ffi threads");
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].id, "ffi-thread");

    let plugin_session = session
        .attach_plugin("ffi-test".into(), Some("ffi-thread".into()))
        .expect("attach ffi plugin");
    assert_eq!(plugin_session.plugin_id, "ffi-test");

    let payload = ReviewPayload {
        summary: Some("Testing".into()),
        ..ReviewPayload::default()
    };
    let submission = session
        .post_review(plugin_session.clone(), payload)
        .expect("post review");
    assert!(submission.revision_started);
    assert_eq!(submission.reference.as_deref(), Some("ffi-ref"));

    let progress = session
        .poll_revision(plugin_session)
        .expect("poll revision");
    assert_eq!(progress.state, RevisionState::Completed);
    assert_eq!(progress.detail.as_deref(), Some("done"));
}
