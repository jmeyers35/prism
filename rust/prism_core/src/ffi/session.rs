use std::sync::{Arc, Mutex};

use crate::{
    diff::DiffEngine,
    plugins::{
        default_registry, PluginService, PluginSession, PluginSummary, ReviewPayload,
        RevisionProgress, SubmissionResult, ThreadRef,
    },
    repository::{Repository, RepositorySnapshot},
    Diff, RepositoryInfo, Revision, RevisionRange, WorkspaceStatus,
};

use super::CoreError;

type Result<T> = std::result::Result<T, CoreError>;

/// High-level handle exposed to Swift via `UniFFI`.
#[derive(Debug)]
pub struct CoreSession {
    repository: Arc<Mutex<Repository>>,
    diff_engine: DiffEngine,
    plugins: Arc<PluginService>,
}

impl CoreSession {
    /// Construct a session for the provided repository path.
    fn new(repository: Repository) -> Self {
        let plugin_service = PluginService::new(default_registry());
        Self {
            repository: Arc::new(Mutex::new(repository)),
            diff_engine: DiffEngine::new(),
            plugins: Arc::new(plugin_service),
        }
    }

    /// Capture the current repository snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when locking the repository fails or when git state cannot be read.
    pub fn snapshot(&self) -> Result<RepositorySnapshot> {
        self.with_repository(Repository::snapshot)
    }

    /// Convenience alias for requesting a fresh snapshot.
    ///
    /// # Errors
    ///
    /// Propagates any error from [`CoreSession::snapshot`].
    pub fn refresh(&self) -> Result<RepositorySnapshot> {
        self.snapshot()
    }

    /// Fetch repository metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if repository information cannot be accessed.
    pub fn repository_info(&self) -> Result<RepositoryInfo> {
        self.with_repository(Repository::info)
    }

    /// Fetch the current workspace status.
    ///
    /// # Errors
    ///
    /// Returns an error if reading workspace status fails.
    pub fn workspace_status(&self) -> Result<WorkspaceStatus> {
        self.with_repository(Repository::workspace_status)
    }

    /// Return the head revision, if the repository has one.
    ///
    /// # Errors
    ///
    /// Returns an error when the repository cannot provide head information.
    pub fn head_revision(&self) -> Result<Option<Revision>> {
        self.with_repository(Repository::head_revision)
    }

    /// Return the inferred base revision for the current head.
    ///
    /// # Errors
    ///
    /// Returns an error when the base revision cannot be determined.
    pub fn base_revision(&self) -> Result<Option<Revision>> {
        self.with_repository(Repository::base_revision)
    }

    /// Generate a diff for the current head/base range.
    ///
    /// # Errors
    ///
    /// Returns an error when diff computation fails or repository access is unavailable.
    pub fn diff_head(&self) -> Result<Diff> {
        let repository = self.repository.lock().map_err(CoreError::from)?;
        self.diff_engine.diff(&repository).map_err(CoreError::from)
    }

    /// Generate a diff representing staged and unstaged workspace changes.
    ///
    /// # Errors
    ///
    /// Returns an error when diff computation fails or repository access is unavailable.
    pub fn diff_workspace(&self) -> Result<Diff> {
        let repository = self.repository.lock().map_err(CoreError::from)?;
        self.diff_engine
            .diff_workspace(&repository)
            .map_err(CoreError::from)
    }

    /// Generate a diff for an explicit revision range.
    ///
    /// # Errors
    ///
    /// Returns an error when diff computation fails or the repository lock is poisoned.
    pub fn diff_for_range(&self, range: RevisionRange) -> Result<Diff> {
        let repository = self.repository.lock().map_err(CoreError::from)?;
        self.diff_engine
            .diff_for_range(&repository, range)
            .map_err(CoreError::from)
    }

    /// List registered plugin summaries for UI presentation.
    #[must_use]
    pub fn plugins(&self) -> Vec<PluginSummary> {
        self.plugins.summaries()
    }

    /// Enumerate threads for a specific plugin.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::PluginNotRegistered`] or wraps plugin failures.
    #[allow(clippy::needless_pass_by_value)]
    pub fn plugin_threads(&self, plugin_id: String) -> Result<Vec<ThreadRef>> {
        self.plugins
            .list_threads(&plugin_id)
            .map_err(CoreError::from)
    }

    /// Attach to a plugin session.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::PluginNotRegistered`] or surfaces plugin failures.
    #[allow(clippy::needless_pass_by_value)]
    pub fn attach_plugin(
        &self,
        plugin_id: String,
        thread_id: Option<String>,
    ) -> Result<PluginSession> {
        self.plugins
            .attach(&plugin_id, thread_id.as_deref())
            .map_err(CoreError::from)
    }

    /// Post review payload through the plugin session.
    ///
    /// # Errors
    ///
    /// Propagates plugin-originated failures.
    #[allow(clippy::needless_pass_by_value)]
    pub fn post_review(
        &self,
        session: PluginSession,
        payload: ReviewPayload,
    ) -> Result<SubmissionResult> {
        self.plugins
            .post_review(&session, payload)
            .map_err(CoreError::from)
    }

    /// Poll revision status for the given session.
    ///
    /// # Errors
    ///
    /// Propagates plugin-originated failures.
    #[allow(clippy::needless_pass_by_value)]
    pub fn poll_revision(&self, session: PluginSession) -> Result<RevisionProgress> {
        self.plugins
            .poll_revision(&session)
            .map_err(CoreError::from)
    }

    fn with_repository<F, T>(&self, op: F) -> Result<T>
    where
        F: FnOnce(&Repository) -> crate::Result<T>,
    {
        let repository = self.repository.lock().map_err(CoreError::from)?;
        op(&repository).map_err(CoreError::from)
    }
}

/// Open a repository session via the `UniFFI` namespace function.
///
/// # Errors
///
/// Returns an error when the repository cannot be opened or fails to initialize.
pub fn open(path: String) -> Result<Arc<CoreSession>> {
    let repository = Repository::open(path).map_err(CoreError::from)?;
    Ok(Arc::new(CoreSession::new(repository)))
}
