# Prism Plugins Crate

`prism_plugins` bundles Prism’s first-party plugin implementations and exposes helpers for constructing registries with sensible defaults.

## Included Plugins

- `GitOnlyPlugin`: lightweight builtin that enables local-only review flows without remote transport
- `AmpPlugin`: CLI-backed integration for Sourcegraph Amp, responsible for thread discovery and review submission

## Registry Helpers

- `default_registry()` seeds a `PluginRegistry` with the builtins plus any test plugins registered through `prism_plugin_api`

Consumers (e.g., `prism_core`) depend on this crate to avoid duplicating plugin wiring logic while still keeping the core crate free from direct Amp CLI concerns.
