use serde::{Deserialize, Serialize};

use prism_api::{CommentDraft, Diagnostic};

/// Capabilities advertised by a plugin for UI feature toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PluginCapabilities {
    /// Whether the plugin can enumerate existing threads.
    pub supports_list_threads: bool,
    /// Whether the plugin can attach without an explicit thread reference.
    pub supports_attach_without_thread: bool,
    /// Whether the plugin supports polling for revision progress.
    pub supports_polling: bool,
}

impl PluginCapabilities {
    /// Construct a new capabilities struct with explicit flags.
    #[must_use]
    pub const fn new(
        supports_list_threads: bool,
        supports_attach_without_thread: bool,
        supports_polling: bool,
    ) -> Self {
        Self {
            supports_list_threads,
            supports_attach_without_thread,
            supports_polling,
        }
    }
}

/// Summary information about a registered plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSummary {
    /// Stable identifier for the plugin.
    pub id: String,
    /// Human-friendly label for display.
    pub label: String,
    /// Capability flags indicating supported flows.
    pub capabilities: PluginCapabilities,
}

/// Lightweight reference to a remote review thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadRef {
    /// Unique identifier for the thread in the plugin backend.
    pub id: String,
    /// Optional display title or summary.
    #[serde(default)]
    pub title: Option<String>,
}

impl ThreadRef {
    /// Construct a new thread reference.
    #[must_use]
    pub fn new(id: impl Into<String>, title: Option<impl Into<String>>) -> Self {
        Self {
            id: id.into(),
            title: title.map(Into::into),
        }
    }
}

/// Handle returned after attaching to a plugin session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSession {
    /// Identifier for the plugin that created the session.
    pub plugin_id: String,
    /// Opaque token understood by the plugin for subsequent calls.
    pub session_id: String,
    /// Optional bound thread reference for the session.
    #[serde(default)]
    pub thread: Option<ThreadRef>,
}

impl PluginSession {
    /// Create a new session handle.
    #[must_use]
    pub fn new(
        plugin_id: impl Into<String>,
        session_id: impl Into<String>,
        thread: Option<ThreadRef>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            session_id: session_id.into(),
            thread,
        }
    }
}

/// Structured payload describing review feedback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReviewPayload {
    /// Optional human summary of the requested revisions.
    #[serde(default)]
    pub summary: Option<String>,
    /// High-level actions the agent should perform.
    #[serde(default)]
    pub actions: Vec<String>,
    /// Draft comments prepared for submission.
    #[serde(default)]
    pub comments: Vec<CommentDraft>,
    /// Diagnostics produced during review.
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

impl ReviewPayload {
    /// Create an empty payload.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Outcome of submitting a review payload to the plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SubmissionResult {
    /// Whether the plugin accepted the submission and started processing.
    #[serde(default)]
    pub revision_started: bool,
    /// Optional reference identifier returned by the plugin backend.
    #[serde(default)]
    pub reference: Option<String>,
    /// Optional informational message for the UI.
    #[serde(default)]
    pub message: Option<String>,
}

/// States reported when polling for revision progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionState {
    /// Work has been queued or is awaiting processing.
    Pending,
    /// Work has started and is in progress.
    InProgress,
    /// Work completed successfully and revisions should be available.
    Completed,
    /// Work failed to complete.
    Failed,
}

impl Default for RevisionState {
    fn default() -> Self {
        Self::Pending
    }
}

/// Poll result describing the latest revision state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RevisionProgress {
    /// Current state of the revision process.
    pub state: RevisionState,
    /// Optional detail or error message surfaced by the plugin.
    #[serde(default)]
    pub detail: Option<String>,
}

/// Errors surfaced by plugin integrations.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// Operation is not supported by the plugin.
    #[error("operation '{operation}' is not supported by this plugin")]
    UnsupportedCapability {
        /// Name of the unsupported operation.
        operation: &'static str,
    },
    /// Generic failure surfaced by the plugin.
    #[error("{message}")]
    Failure {
        /// Human-readable error message.
        message: String,
    },
}

impl PluginError {
    /// Helper to construct a failure from any displayable message.
    #[must_use]
    pub fn message(message: impl Into<String>) -> Self {
        Self::Failure {
            message: message.into(),
        }
    }
}

/// Convenience result alias for plugin operations.
pub type PluginResult<T> = std::result::Result<T, PluginError>;
