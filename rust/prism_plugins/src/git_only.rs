use std::sync::atomic::{AtomicU64, Ordering};

use prism_plugin_api::{
    AgentPlugin, PluginCapabilities, PluginResult, PluginSession, ReviewPayload, RevisionProgress,
    RevisionState, SubmissionResult, ThreadRef,
};

static GIT_ONLY_SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Minimal builtin plugin that performs no external calls.
#[derive(Debug, Default)]
pub struct GitOnlyPlugin;

impl AgentPlugin for GitOnlyPlugin {
    fn id(&self) -> &'static str {
        "git-only"
    }

    fn label(&self) -> &'static str {
        "Local Git"
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities::new(false, true, false)
    }

    fn list_threads(&self) -> PluginResult<Vec<ThreadRef>> {
        Ok(Vec::new())
    }

    fn attach(&self, thread_id: Option<&str>) -> PluginResult<PluginSession> {
        let session_id = format!(
            "local-{}",
            GIT_ONLY_SESSION_COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        let thread = thread_id.map(|id| ThreadRef::new(id, None::<String>));
        Ok(PluginSession::new(self.id(), session_id, thread))
    }

    fn post_review(
        &self,
        _session: &PluginSession,
        _payload: ReviewPayload,
    ) -> PluginResult<SubmissionResult> {
        Ok(SubmissionResult {
            revision_started: false,
            reference: None,
            message: Some("No remote submission performed for local reviews.".into()),
        })
    }

    fn poll_revision(&self, _session: &PluginSession) -> PluginResult<RevisionProgress> {
        Ok(RevisionProgress {
            state: RevisionState::Completed,
            detail: Some("Revisions are managed locally.".into()),
        })
    }
}
