# Workspace URL Launchers

## Goal

Activate the existing `Workspace.urls` configuration as lightweight launch shortcuts. URLs open in the macOS default browser on the first workspace activation during each boot without becoming Ctx-owned tabs or windows.

Git automation, services, pause notes, custom URL schemes, per-workspace browsers, and the Tauri UI are deferred.

## Product Rules

- [x] Accept and normalize only credential-free HTTP and HTTPS URLs.
- [x] Keep the existing `urls: [String]` YAML representation backward compatible.
- [x] Open pending URLs after window recovery and before target-window focus.
- [x] Launch each configured URL once per macOS boot unless explicitly forced.
- [x] Treat URLs already captured in Firefox recovery state as recovery-managed.
- [x] Retry failed launches without rolling back an otherwise successful workspace switch.
- [x] Never close, hide, position, or claim ownership of browser tabs opened from URL shortcuts.

## Step 1: Model and Runtime State

- [x] Add URL parsing, normalization, validation, status, and launch-report types.
- [x] Add backward-compatible boot-session URL state to `runtime.json`.
- [x] Reset successful-launch markers when the macOS boot identifier changes.
- [x] Add unit tests for validation, legacy state, boot changes, and marker cleanup.

## Step 2: CLI Management

- [x] Add `ctx url add <workspace> <url...>`.
- [x] Add `ctx url remove <workspace> <url...>`.
- [x] Add `ctx url list [workspace]`.
- [x] Add `ctx url open [workspace] [--force]`.
- [x] Include URL statuses in text and JSON `show`, `status`, and URL commands.
- [x] Make duplicate additions idempotent and removals atomic.

## Step 3: Workspace Activation

- [x] Launch pending URLs through `/usr/bin/open` using the default browser.
- [x] Integrate URL launch after recovery matching and before final window restore/focus.
- [x] Persist only successful launches and retry failures later.
- [x] Suppress launches already covered by Firefox recovery tabs.
- [x] Keep switch successful when URL launching is partially degraded.
- [x] Clear matching runtime markers from `url remove` and workspace removal.

## Step 4: Verification

- [x] Run `cargo fmt --all`.
- [x] Run `cargo test --workspace`.
- [x] Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] Run `cargo build --workspace`.
- [x] Run `git diff --check`.
- [x] Run `graphify update .`.
- [x] Complete a disposable localhost Computer Use smoke test without leaving config or runtime markers.
- [x] Commit the model, CLI, switch integration, and final verification as checkpoints.

## Acceptance

The first activation opens configured URLs, repeated activation in the same boot opens none, `--force` opens them again, target-window focus wins after browser launch, Firefox recovery does not duplicate matching URLs, and all temporary smoke-test state is removed.
