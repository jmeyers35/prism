//! Plugin registry keeps track of available agent integrations.

use std::collections::HashMap;

use super::AgentPlugin;

/// In-memory registry for agent plugins.
#[derive(Default)]
pub struct PluginRegistry {
    plugins: HashMap<&'static str, Box<dyn AgentPlugin>>,
}

impl PluginRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a plugin keyed by its `AgentPlugin::id`.
    pub fn register<P>(&mut self, plugin: P)
    where
        P: AgentPlugin + 'static,
    {
        self.plugins.insert(plugin.id(), Box::new(plugin));
    }

    /// Retrieve a plugin by identifier.
    pub fn get(&self, id: &str) -> Option<&dyn AgentPlugin> {
        self.plugins.get(id).map(|plugin| plugin.as_ref())
    }

    /// Returns the list of registered plugin identifiers.
    pub fn ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.plugins.keys().copied()
    }
}
