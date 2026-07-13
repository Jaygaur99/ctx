# macOS Desktop Placement

## Goal

Remember which macOS Desktop/Space contains every workspace window and return both existing and recovered windows to the corresponding Desktop during `ctx switch`.

The implementation remains local-first and native Rust. A Tauri application is not required. Interacting with Spaces requires private macOS SkyLight APIs, so this feature must fail safely when an OS update changes or removes those APIs.

## Product Rules

- [x] Persist a stable placement description instead of relying only on the opaque live Space ID.
- [x] Treat Desktop placement as best-effort metadata; failure must not corrupt recovery state or make ordinary switching unusable.
- [x] Preserve placement independently for every window in a workspace.
- [x] Support multiple displays by recording the display and the Desktop's ordinal on that display.
- [x] Move an existing window before restoring/focusing it.
- [x] Move a newly recovered window after it is matched and before restoring/focusing it.
- [x] Never move ignored or unassigned windows.
- [x] Report degraded placement clearly in `snapshot`, `show`, `status`, and JSON output.

## Step 1: Native Space Inventory

- [x] Add a macOS-only `spaces` module behind a typed API boundary.
- [x] Dynamically load the required SkyLight symbols so unsupported macOS versions produce a typed error instead of a linker or startup failure.
- [x] Enumerate managed displays, ordered user Desktops, and the current Desktop, with a read-only `com.apple.spaces` fallback when SkyLight returns no inventory.
- [x] Query the Space membership for a Core Graphics window ID.
- [x] Filter fullscreen/system Spaces from user Desktop ordinals.
- [x] Add pure unit tests for inventory normalization and ordinal mapping.
- [x] Add `ctx spaces [window-id]` with text and JSON output to inspect placement before enabling mutation.

Smoke gate: two windows placed on different existing Desktops are reported with different Desktop ordinals, and repeated inspection is stable.

## Step 2: Capture and Persistence

- [x] Add optional placement metadata to `WindowInfo` with backward-compatible Serde defaults.
- [x] Capture placement when a window is added to a workspace.
- [x] Refresh placement during `ctx snapshot` without discarding the previous valid placement on a transient query failure.
- [x] Preserve placement when stale window IDs are reconciled.
- [x] Show placement and degradation in text and JSON output.
- [x] Add config round-trip and backward-compatibility tests.

Smoke gate: clearing and recreating a workspace records the correct display and Desktop ordinal for every selected window.

## Step 3: Restore to Existing Desktops

- [x] Add a typed operation for moving a window to an existing managed Space.
- [x] Integrate placement after recovery matching and before window restore/focus.
- [x] Apply placement to windows that never closed as well as newly recovered windows.
- [x] Keep the current active Desktop stable while moving windows.
- [x] Treat placement failure as degraded metadata without rolling back a successful app recovery or ordinary switch.
- [x] Make repeated `ctx switch` idempotent and avoid duplicate app recovery.

Smoke gate: VS Code, Firefox, and Warp recover onto their recorded existing Desktops and a second switch creates no windows and performs no unnecessary moves.

## Step 4: Create Missing Desktops

- [ ] Detect when the recorded Desktop ordinal no longer exists on its display.
- [ ] Create only the minimum number of missing Desktops.
- [ ] Re-enumerate Spaces after each creation instead of assuming new IDs or ordering.
- [ ] Refuse to remove or reorder user Desktops.
- [ ] Add a capability check and an explicit warning when Desktop creation is unsupported on the current macOS release.

Smoke gate: with one Desktop present and a window recorded on Desktop 3, Ctx creates two Desktops and restores the window onto Desktop 3.

## Step 5: End-to-End Acceptance

- [ ] Test VS Code, Firefox, and Warp across at least three Desktops.
- [ ] Close all three app windows, switch away, then recover the workspace and verify app context plus placement.
- [ ] Repeat the switch and verify there are no duplicates.
- [ ] Test a missing display and document the fallback behavior.
- [ ] Run `cargo test --workspace`.
- [ ] Run `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] Run `cargo build --workspace`.
- [ ] Run `graphify update .`.
- [ ] Commit each completed step as its own checkpoint.

## Explicitly Deferred

- Moving windows between physical displays when the original display is disconnected.
- Renaming or deleting macOS Desktops.
- Restoring Mission Control ordering beyond each display's Desktop ordinal.
- A Tauri desktop UI.
