# Prism Plugin API Crate

`prism_plugin_api` defines the traits and lightweight registry helpers that Prism plugins implement. It couples the shared review data types from `prism_api` with plugin-facing abstractions so individual integrations can live outside the core crate.

## Key Pieces

- `AgentPlugin` trait: capability flags, attachment hooks, review submission, and revision polling contract
- `PluginRegistry`: in-memory registry for plugin discovery, plus helpers for test-only registrations
- Transport-safe wrapper types such as `PluginSession`, `ReviewPayload`, `RevisionProgress`, and `ThreadRef`

## When to Use

- Implement a new plugin by depending on `prism_plugin_api` and providing an `AgentPlugin` implementation
- Share test plugins across crates via `register_test_plugin`
- Access plugin metadata from consumer crates (e.g., to surface `PluginSummary` data to UI layers)
