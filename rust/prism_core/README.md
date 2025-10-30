# prism_core

Rust core library for Prism. This crate provides the foundational building blocks that power the macOS review app:

- repository access and snapshot management
- diff generation and patch application
- agent plugin registration for external tooling (e.g., Amp)

Current scope tracks the first wave of tasks:

- `prism-20`: crate scaffold and module layout (this change)
- `prism-19`: API types + serde-friendly data models
- `prism-5`: libgit2 repository integration
- `prism-24`: agent plugin protocol and registry

## Development

Use [`mise`](https://mise.jdx.dev) for toolchain installation and repeatable tasks:

```bash
# ensure the Rust toolchain and commands are installed
mise use

# build, test, or lint the crate
mise run build-core
mise run test-core
mise run clippy-core
```
