//! Agent plugin system entry points.

mod amp;
mod builtin;
mod registry;
mod service;
mod types;

pub use amp::AmpPlugin;
pub use builtin::GitOnlyPlugin;
pub use registry::{register_test_plugin, PluginRegistry, TestPluginRegistration};
pub use service::PluginService;
pub use types::*;

/// Trait implemented by agent integrations (e.g., Amp).
pub trait AgentPlugin: Send + Sync {
    /// Stable identifier used for lookup and logging.
    fn id(&self) -> &'static str;

    /// Human-friendly label for UI surfaces.
    fn label(&self) -> &'static str;

    /// Capabilities advertised by the plugin.
    fn capabilities(&self) -> PluginCapabilities;

    /// Enumerate review threads available to the plugin.
    ///
    /// # Errors
    ///
    /// Implementors should surface any transport or backend failures.
    fn list_threads(&self) -> PluginResult<Vec<ThreadRef>>;

    /// Attach to a thread and obtain a session handle.
    ///
    /// # Errors
    ///
    /// Returns plugin-defined errors when attachment fails.
    fn attach(&self, thread_id: Option<&str>) -> PluginResult<PluginSession>;

    /// Submit a review payload for the active session.
    ///
    /// # Errors
    ///
    /// Returns plugin-defined errors when submission fails.
    fn post_review(
        &self,
        session: &PluginSession,
        payload: ReviewPayload,
    ) -> PluginResult<SubmissionResult>;

    /// Poll for revision progress for the session.
    ///
    /// # Errors
    ///
    /// Returns plugin-defined errors when polling fails.
    fn poll_revision(&self, session: &PluginSession) -> PluginResult<RevisionProgress>;
}
