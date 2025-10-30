//! Agent plugin system entry points.

mod registry;

pub use registry::PluginRegistry;

/// Trait implemented by agent integrations (e.g., Amp).
pub trait AgentPlugin: Send + Sync {
    /// Stable identifier used for lookup and logging.
    fn id(&self) -> &'static str;

    /// Human-friendly label for UI surfaces.
    fn label(&self) -> &'static str;
}
