//! Agent plugin system entry points.

mod service;

pub use prism_plugin_api::{
    register_test_plugin, AgentPlugin, PluginCapabilities, PluginError, PluginRegistry,
    PluginResult, PluginSession, PluginSummary, ReviewPayload, RevisionProgress, RevisionState,
    SubmissionResult, TestPluginRegistration, ThreadRef,
};
pub use prism_plugins::{default_registry, AmpPlugin, GitOnlyPlugin};

pub use service::PluginService;
