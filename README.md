# Prism

Native macOS app for PR‑style review of agent‑created diffs with a tight “review → feedback → revise → refresh” loop. v0 targets Sourcegraph’s Amp and can attach to an existing Amp thread without restarting work. Unified diff view only for MVP.

## Why
- Trust and control before merging: inspect diffs locally, annotate, and iterate with your agent.
- Amp thread‑linked flow keeps the loop tight: send structured feedback to an existing thread and watch revisions land.

## Integration
- Thread‑linked attach (Amp)
  - Bind to an existing Amp thread (paste ID, or pick from a list when available).
  - Post review payload to the thread, show “Revising…”, refresh diffs when files change or the CLI run completes.

## Core UX (MVP)
- Files changed list + unified diffs (per‑file collapsed, first few hunks auto‑expanded).
- Inline comments with quick labels (bug/test/style/perf).
- Suggestion blocks rendered as patch hunks; dry‑run apply before writing.
- Review composer sends a structured payload to your agent.
- Manual comment resolution (no auto‑resolve for v0).

## Amp Integration (CLI)
Prism talks to Amp via its CLI. Key surfaces discovered via `amp -h`:

- Create/attach/list threads
  - `amp threads new`
  - `amp threads continue <threadId>` (interactive picker if `<threadId>` omitted; `--last` uses most recent)
  - `amp threads list` (lists threads; parse for recent selection)
- One‑shot execution and streaming
  - `-x, --execute [message]` to run a single message and exit.
  - `--stream-json` to emit Claude‑Code‑compatible streaming JSON.
  - `--stream-json-input` to read JSONL user messages from stdin (with `--execute` + `--stream-json`).

Examples Prism will use under the hood:

1) Post a plain text review message to a thread (simple case)

```bash
echo "Please address the following review.\n\n\`\`\`json\n{<review-payload>}\n\`\`\`" \
| amp threads continue <THREAD_ID> -x
```

2) Post structured JSON as a user message (JSON‑lines input)

```bash
printf '%s\n' \
'{"role":"user","content":[{"type":"text","text":"Review payload follows as JSON."}]}' \
"$(jq -c --argjson p @review.json '{role:"user",content:[{type:"text",text:($p|tojson)}]}')" \
| amp threads continue <THREAD_ID> -x --stream-json --stream-json-input
```

Notes
- `amp threads list` may require network; Prism degrades to paste‑ID mode when offline or unauthenticated.
- For “Revising…”, Prism either waits for the one‑shot `amp ... -x` process to complete or watches the filesystem for changes.

## Feedback Payload (MVP)
Prism sends this JSON in the review message (inline or as JSONL content). Anchoring is simple for v0 (path + 1‑based new line); we’ll iterate as needed.

```json
{
  "review_summary": "One-paragraph intent and priorities",
  "actions": ["fix-tests", "reduce-diff-scope", "tighten-types"],
  "comments": [
    {
      "path": "src/foo.ts",
      "line_new": 42,
      "severity": "blocker|nit",
      "note": "Type mismatch; tighten generics.",
      "suggestion_patch": "optional unified-diff hunk"
    }
  ]
}
```

## Architecture
- App: SwiftUI (macOS), keyboard‑first controls (j/k next/prev, c comment, r request changes, a approve, Cmd+Enter send, o open in editor).
- Git: libgit2 via SwiftGit2 for diffs, hunks, and patch apply; 3‑way dry‑run before writing. Honors `.gitignore`.
- Amp Adapter: CLI wrapper for thread attach, one‑shot posts, and optional streaming.
- Storage: Core Data (SQLite) for sessions, comments, iterations, thread link, and base snapshot.
- File Watching: FSEvents/DispatchSource for snappy auto‑refresh when files change.

## Development
- Requirements
  - macOS 13+
  - Xcode 15+
  - Amp CLI installed and authenticated (`amp login` or `AMP_API_KEY`)
- Build
  - Open the Xcode project (to be added) and run on macOS.
  - Dependencies via Swift Package Manager: SwiftGit2 (+ bundled libgit2).

### Rust FFI bindings
- `mise swift-bindings` regenerates the UniFFI Swift glue code under `swift/PrismFFI/Sources/PrismFFI/`.
- `mise build-xcframework` builds an arm64-only `PrismCoreFFI.xcframework` in `swift/PrismFFI/` for consumption via Swift Package Manager. Run `rustup target add aarch64-apple-darwin` once before invoking if the target is missing. (Intel support will be added later if needed.)
- Add `swift/PrismFFI` as a local Swift package in Xcode to pull in the binary target and generated Swift wrapper.

## Performance
- Lazy‑load file diffs; virtualize long files; cache parsed hunks.
- Soft cap large files (e.g., skip >500KB unless expanded) to keep the UI responsive.
- Target: time‑to‑first‑diff under ~10s on medium repos.

## Security & Privacy
- Local‑first design. Only user‑initiated Amp calls.
- No telemetry. Local Core Data store in `Application Support/Prism`.

## Conductor Parity (Claude Code Sessions)
- Mirror the Conductor loop: attach session → review unified diff → send structured feedback → show revising → refresh diffs.
- Keep finalize light (no commit/merge automation in v0).

## Roadmap (Post‑MVP)
- Split diff view; robust anchors (ranges, commit OIDs, hunk IDs) and auto‑resolve.
- Recent thread discovery (parse `amp threads list` when available).
- Apply‑suggestion conflict UI with minimal 3‑way context and “open in editor”.
- Optional PR open via `gh` if requested by users.
