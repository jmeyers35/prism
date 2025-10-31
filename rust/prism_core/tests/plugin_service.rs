use std::sync::{Arc, Mutex};

use prism_core::plugins::{
    AgentPlugin, PluginCapabilities, PluginError, PluginRegistry, PluginService, PluginSession,
    ReviewPayload, RevisionProgress, RevisionState, SubmissionResult, ThreadRef,
};

#[derive(Default)]
struct FakePlugin {
    attachments: Arc<Mutex<Vec<Option<String>>>>,
    submissions: Arc<Mutex<Vec<String>>>,
}

impl AgentPlugin for FakePlugin {
    fn id(&self) -> &'static str {
        "fake"
    }

    fn label(&self) -> &'static str {
        "Fake Plugin"
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities::new(true, true, true)
    }

    fn list_threads(&self) -> Result<Vec<ThreadRef>, PluginError> {
        Ok(vec![ThreadRef::new("thread-1", Some("Sample Thread"))])
    }

    fn attach(&self, thread_id: Option<&str>) -> Result<PluginSession, PluginError> {
        self.attachments
            .lock()
            .expect("attachments lock")
            .push(thread_id.map(str::to_owned));
        Ok(PluginSession::new(
            self.id(),
            "session-1",
            thread_id.map(|id| ThreadRef::new(id, Some("Sample Thread"))),
        ))
    }

    fn post_review(
        &self,
        _session: &PluginSession,
        payload: ReviewPayload,
    ) -> Result<SubmissionResult, PluginError> {
        self.submissions
            .lock()
            .expect("submissions lock")
            .push(payload.summary.unwrap_or_default());
        Ok(SubmissionResult {
            revision_started: true,
            reference: Some("rev-1".into()),
            message: Some("Processing".into()),
        })
    }

    fn poll_revision(&self, _session: &PluginSession) -> Result<RevisionProgress, PluginError> {
        Ok(RevisionProgress {
            state: RevisionState::Completed,
            detail: Some("Done".into()),
        })
    }
}

#[test]
fn plugin_service_round_trip() {
    let mut registry = PluginRegistry::new();
    registry.register(FakePlugin::default());

    let service = PluginService::new(registry);

    let summaries = service.summaries();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, "fake");

    let threads = service.list_threads("fake").expect("list threads");
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].id, "thread-1");

    let session = service.attach("fake", Some("thread-1")).expect("attach");
    assert_eq!(session.plugin_id, "fake");
    assert!(session.thread.is_some());

    let payload = ReviewPayload {
        summary: Some("Looks good".into()),
        ..ReviewPayload::new()
    };
    let submission = service.post_review(&session, payload).expect("post review");
    assert!(submission.revision_started);
    assert_eq!(submission.reference.as_deref(), Some("rev-1"));

    let progress = service.poll_revision(&session).expect("poll revision");
    assert_eq!(progress.state, RevisionState::Completed);
}

#[test]
fn missing_plugin_is_reported() {
    let service = PluginService::new(PluginRegistry::new());
    let err = service.list_threads("missing").expect_err("missing plugin");
    match err {
        prism_core::Error::PluginNotRegistered { plugin } => {
            assert_eq!(plugin, "missing");
        }
        other => panic!("expected PluginNotRegistered, got {:?}", other),
    }
}
