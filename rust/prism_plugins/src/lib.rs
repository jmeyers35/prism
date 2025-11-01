mod amp;
mod git_only;

pub use amp::AmpPlugin;
pub use git_only::GitOnlyPlugin;

use prism_plugin_api::{registered_test_plugins, PluginRegistry};

/// Build a plugin registry populated with Prism's default integrations.
#[must_use]
pub fn default_registry() -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    registry.register(GitOnlyPlugin);
    registry.register(AmpPlugin::default());

    for plugin in registered_test_plugins() {
        registry.register_arc(plugin);
    }

    registry
}
