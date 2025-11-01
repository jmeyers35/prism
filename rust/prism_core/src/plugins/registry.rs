//! Plugin registry keeps track of available agent integrations.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};

use super::{AgentPlugin, AmpPlugin, GitOnlyPlugin, PluginCapabilities, PluginSummary};

static TEST_PLUGIN_STORE: OnceLock<Mutex<Vec<Arc<dyn AgentPlugin>>>> = OnceLock::new();

fn test_plugins() -> &'static Mutex<Vec<Arc<dyn AgentPlugin>>> {
    TEST_PLUGIN_STORE.get_or_init(|| Mutex::new(Vec::new()))
}

/// In-memory registry for agent plugins.
#[derive(Default)]
pub struct PluginRegistry {
    plugins: HashMap<String, Arc<dyn AgentPlugin>>,
}

impl PluginRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Instantiate a registry pre-populated with built-in plugins.
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(GitOnlyPlugin);
        registry.register(AmpPlugin::default());
        if let Ok(plugins) = test_plugins().lock() {
            for plugin in plugins.iter() {
                registry.register_arc(plugin.clone());
            }
        }
        registry
    }

    /// Register a plugin keyed by its `AgentPlugin::id`.
    pub fn register<P>(&mut self, plugin: P)
    where
        P: AgentPlugin + 'static,
    {
        let id = plugin.id();
        self.plugins.insert(id.to_string(), Arc::new(plugin));
    }

    /// Register an already shared plugin instance.
    pub fn register_arc(&mut self, plugin: Arc<dyn AgentPlugin>) {
        let id = plugin.id().to_string();
        self.plugins.insert(id, plugin);
    }

    /// Retrieve a plugin by identifier.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<Arc<dyn AgentPlugin>> {
        self.plugins.get(id).cloned()
    }

    /// Returns the list of registered plugin identifiers.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.plugins.keys().map(String::as_str)
    }

    /// Build summaries for all registered plugins.
    #[must_use]
    pub fn summaries(&self) -> Vec<PluginSummary> {
        self.plugins
            .values()
            .map(|plugin| PluginSummary {
                id: plugin.id().to_string(),
                label: plugin.label().to_string(),
                capabilities: plugin.capabilities(),
            })
            .collect()
    }

    /// Fetch capabilities for a specific plugin.
    #[must_use]
    pub fn capabilities(&self, id: &str) -> Option<PluginCapabilities> {
        self.plugins.get(id).map(|plugin| plugin.capabilities())
    }
}

impl fmt::Debug for PluginRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let plugin_ids: Vec<&str> = self.plugins.keys().map(String::as_str).collect();
        f.debug_struct("PluginRegistry")
            .field("plugins", &plugin_ids)
            .finish()
    }
}

/// Guard returned when registering a test plugin to ensure cleanup on drop.
#[derive(Debug)]
pub struct TestPluginRegistration {
    id: String,
}

impl Drop for TestPluginRegistration {
    fn drop(&mut self) {
        if let Ok(mut plugins) = test_plugins().lock() {
            if let Some(position) = plugins.iter().position(|plugin| plugin.id() == self.id) {
                plugins.remove(position);
            }
        }
    }
}

/// Register a plugin instance for tests so it appears in newly created registries.
///
/// The plugin remains active until the returned guard is dropped.
pub fn register_test_plugin<P>(plugin: P) -> TestPluginRegistration
where
    P: AgentPlugin + 'static,
{
    let id = plugin.id().to_string();
    let plugin: Arc<dyn AgentPlugin> = Arc::new(plugin);
    let mut plugins = match test_plugins().lock() {
        Ok(plugins) => plugins,
        Err(poisoned) => poisoned.into_inner(),
    };
    plugins.retain(|existing| existing.id() != id);
    plugins.push(plugin);
    drop(plugins);

    TestPluginRegistration { id }
}
