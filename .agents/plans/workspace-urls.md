# Workspace URL Launchers

## Goal

Activate the existing `Workspace.urls` configuration as lightweight launch shortcuts. URLs open in the macOS default browser on the first workspace activation during each boot without becoming Ctx-owned tabs or windows.

Git automation, services, pause notes, custom URL schemes, per-workspace browsers, and the Tauri UI are deferred.

## Product Rules

- [ ] Accept and normalize only credential-free HTTP and HTTPS URLs.
- [ ] Keep the existing `urls: [String]` YAML representation backward compatible.
- [ ] Open pending URLs after window recovery and before target-window focus.
- [ ] Launch each configured URL once per macOS boot unless explicitly forced.
- [ ] Treat URLs already captured in Firefox recovery state as recovery-managed.
- [ ] Retry failed launches without rolling back an otherwise successful workspace switch.
- [ ] Never close, hide, position, or claim ownership of browser tabs opened from URL shortcuts.

## Step 1: Model and Runtime State

- [ ] Add URL parsing, normalization, validation, status, and launch-report types.
- [ ] Add backward-compatible boot-session URL state to `runtime.json`.
- [ ] Reset successful-launch markers when the macOS boot identifier changes.
- [ ] Add unit tests for validation, legacy state, boot changes, and marker cleanup.

## Step 2: CLI Management

- [ ] Add `ctx url add <workspace> <url...>`.
- [ ] Add `ctx url remove <workspace> <url...>`.
- [ ] Add `ctx url list [workspace]`.
- [ ] Add `ctx url open [workspace] [--force]`.
- [ ] Include URL statuses in text and JSON `show`, `status`, and URL commands.
- [ ] Make duplicate additions idempotent and removals atomic.

## Step 3: Workspace Activation

- [ ] Launch pending URLs through `/usr/bin/open` using the default browser.
- [ ] Integrate URL launch after recovery matching and before final window restore/focus.
- [ ] Persist only successful launches and retry failures later.
- [ ] Suppress launches already covered by Firefox recovery tabs.
- [ ] Keep switch successful when URL launching is partially degraded.
- [ ] Clear matching runtime markers from `url remove` and workspace removal.

## Step 4: Verification

- [ ] Run `cargo fmt --all`.
- [ ] Run `cargo test --workspace`.
- [ ] Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] Run `cargo build --workspace`.
- [ ] Run `git diff --check`.
- [ ] Run `graphify update .`.
- [ ] Complete a disposable localhost Computer Use smoke test without leaving config or runtime markers.
- [ ] Commit the model, CLI, switch integration, and final verification as checkpoints.

## Acceptance

The first activation opens configured URLs, repeated activation in the same boot opens none, `--force` opens them again, target-window focus wins after browser launch, Firefox recovery does not duplicate matching URLs, and all temporary smoke-test state is removed.
