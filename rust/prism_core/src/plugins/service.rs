use std::sync::Arc;

use super::{
    AgentPlugin, PluginRegistry, PluginResult, PluginSession, PluginSummary, ReviewPayload,
    RevisionProgress, SubmissionResult, ThreadRef,
};
use crate::{Error, Result};

/// High-level façade for invoking plugin operations.
#[derive(Clone)]
pub struct PluginService {
    registry: Arc<PluginRegistry>,
}

impl PluginService {
    /// Create a plugin service backed by the provided registry.
    #[must_use]
    pub fn new(registry: PluginRegistry) -> Self {
        Self {
            registry: Arc::new(registry),
        }
    }

    /// Access the underlying registry.
    #[must_use]
    pub fn registry(&self) -> Arc<PluginRegistry> {
        Arc::clone(&self.registry)
    }

    /// List summaries for all registered plugins.
    #[must_use]
    pub fn summaries(&self) -> Vec<PluginSummary> {
        self.registry.summaries()
    }

    /// Fetch capabilities for a plugin, if registered.
    #[must_use]
    pub fn capabilities(&self, plugin_id: &str) -> Option<super::PluginCapabilities> {
        self.registry.capabilities(plugin_id)
    }

    /// Enumerate threads for the specified plugin.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PluginNotRegistered`] when the id is unknown or propagates plugin failures.
    pub fn list_threads(&self, plugin_id: &str) -> Result<Vec<ThreadRef>> {
        let plugin = self.plugin(plugin_id)?;
        Self::invoke(plugin_id, plugin.list_threads())
    }

    /// Attach to a plugin thread and return the session handle.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PluginNotRegistered`] or plugin-sourced errors.
    pub fn attach(&self, plugin_id: &str, thread_id: Option<&str>) -> Result<PluginSession> {
        let plugin = self.plugin(plugin_id)?;
        Self::invoke(plugin_id, plugin.attach(thread_id))
    }

    /// Submit the review payload through the plugin.
    ///
    /// # Errors
    ///
    /// Propagates failures returned by the plugin.
    pub fn post_review(
        &self,
        session: &PluginSession,
        payload: ReviewPayload,
    ) -> Result<SubmissionResult> {
        let plugin = self.plugin(&session.plugin_id)?;
        Self::invoke(&session.plugin_id, plugin.post_review(session, payload))
    }

    /// Poll for revision progress for the existing session.
    ///
    /// # Errors
    ///
    /// Propagates plugin errors.
    pub fn poll_revision(&self, session: &PluginSession) -> Result<RevisionProgress> {
        let plugin = self.plugin(&session.plugin_id)?;
        Self::invoke(&session.plugin_id, plugin.poll_revision(session))
    }

    fn plugin(&self, plugin_id: &str) -> Result<Arc<dyn AgentPlugin>> {
        self.registry
            .get(plugin_id)
            .ok_or_else(|| Error::PluginNotRegistered {
                plugin: plugin_id.to_string(),
            })
    }

    fn invoke<T>(plugin_id: &str, result: PluginResult<T>) -> Result<T> {
        result.map_err(|source| Error::Plugin {
            plugin: plugin_id.to_string(),
            source,
        })
    }
}

impl std::fmt::Debug for PluginService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let plugin_ids: Vec<String> = self
            .registry
            .ids()
            .map(std::string::ToString::to_string)
            .collect();
        f.debug_struct("PluginService")
            .field("plugins", &plugin_ids)
            .finish()
    }
}
