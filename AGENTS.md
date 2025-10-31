# Repository Guidelines

## Project Structure & Module Organization
- `swift/PrismApp/`: SwiftUI macOS app (UI, Core Data, file watching).
- `rust/prism_core/`: Rust core library (git/diff/patch, agent plugins, FFI).
- `swift/PrismFFI/`: Thin Swift module wrapping the Rust xcframework.
- `docs/`: Product docs (e.g., README), design notes.
- `.beads/`: bd project database and history.

## Build, Test, and Development Commands
- Swift (app): open in Xcode and run locally (macOS 13+).
- Rust (core): `cd rust/prism_core && cargo build --release`.
- Tests: `cargo test` (Rust), `Cmd+U` in Xcode (Swift/XCTest).
- Prefer running tests through the curated `mise` tasks (e.g., `mise test`, `mise clippy-core`) so everyone exercises the same entry points. When new flows need additional automation, add a task to `.mise.toml` rather than relying on ad-hoc shell commands.
- Lint: `mise clippy-core` must pass for every change touching the Rust core.
- UniFFI Swift bindings: `mise swift-bindings` emits generated sources into `swift/PrismFFI/Sources/PrismFFI/`.
- Amp CLI (optional dev): `amp -h`, `amp threads list` for local verification.

## Coding Style & Naming Conventions
- Swift: 2‑space indent, CamelCase types, lowerCamelCase members. Prefer SwiftUI/Combine patterns.
- Rust: `rustfmt` + `clippy` clean; modules `snake_case`, types `CamelCase`.
- Paths: modules under `swift/` and `rust/` mirror feature names (e.g., `DiffView`, `plugins/amp`).

## Testing Guidelines
- Rust: unit tests per module; focus on diff generation, patch apply, and plugin behaviors (Amp/Git‑only).
- Swift: UI logic in view models; add XCTests for payload building and FFI adapters.
- Aim for fast tests; snapshot or golden diffs where helpful.

## Commit & Pull Request Guidelines
- Commits: imperative mood, concise scope (e.g., "core: add unified diff hunks").
- Reference bd issue IDs in PR titles/bodies (e.g., `prism-6`).
- PRs include: purpose, approach, screenshots (UI), and test notes.

## Architecture Overview
- UI (SwiftUI) is agent‑agnostic.
- Core (Rust) owns git/libgit2, diff/patch, and an agent plugin abstraction, exposed to Swift via FFI.
- Agent plugins (e.g., Amp) are implemented in Rust and registered via a plugin registry; Swift calls into Rust only.

## Work Tracking with beads/bd
- Initialized with prefix `prism`. Common commands:
  - `bd ready` (next up), `bd list`, `bd dep tree prism-1`.
  - Create/link: `bd create "Task" -t task -p 1`, `bd dep add <child> <parent> --type parent-child`.
- Reference bd IDs in commits/PRs. Keep acceptance criteria/design notes updated via `bd update`.
