# Prism API Crate

`prism_api` houses the shared data models that Prism exports through FFI and reuses across Rust crates. The types are serde-serializable and intentionally limited to FFI-friendly primitives so they can be surfaced to Swift.

## Contents

- Diff structures (`Diff`, `DiffFile`, `DiffHunk`, etc.) used for presenting repository changes
- Repository metadata (`RepositoryInfo`, `Revision`, `WorkspaceStatus`) shared between the core and clients
- Review-oriented models (`ReviewPayload`, `CommentDraft`, `Diagnostic`, …) leveraged by plugins and the app

## Usage

Any crate within the workspace can depend on `prism_api` for canonical definitions of review payloads or diff metadata. Serde defaults and enums are already configured for external serialization, so calling code can round-trip JSON without custom glue.
