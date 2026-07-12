# Generic App Recovery — Implementation Steps

## Goal

Recover closed applications and their durable context when switching to a workspace. The first release supports VS Code, Antigravity, Warp, and Firefox, with generic relaunch support for other macOS apps.

“Old state” includes application identity, window bounds, editor project, terminal tab directories, and browser tabs. It does not include macOS Desktop placement, unsaved buffers, transient UI state, or rerunning terminal commands.

## Step 0 — Prepare the Work

- [x] Create and switch to the `cli-app-recovery` branch.
- [x] Run the existing test suite and save the clean baseline.
- [ ] Record a small manual test workspace containing VS Code, Antigravity, Warp, and Firefox.

Done when the branch exists and all current tests pass without changing behavior.

## Step 1 — Extend the Saved Window Model

- [x] Add optional stable application identity fields:
  - Bundle ID.
  - Application path.
- [x] Add a tagged recovery state:
  - `editor`.
  - `terminal`.
  - `browser`.
  - `generic`.
- [x] Add an optional capture/recovery warning.
- [x] Keep all new fields optional so existing YAML remains valid.
- [x] Ensure stale window-ID reconciliation updates runtime fingerprints without replacing saved recovery data.
- [x] Add serialization and backward-compatibility tests.

Done when old YAML loads unchanged and new recovery data round-trips through YAML.

## Step 2 — Define the Recovery Adapter Interface

- [x] Introduce a recovery adapter abstraction with operations to:
  - Identify whether the adapter supports an application bundle ID.
  - Capture durable state from a tracked window.
  - Launch or restore the saved state.
  - Help match the newly created window.
- [x] Add an adapter registry keyed by bundle ID.
- [x] Add fake adapters and a fake platform implementation for adapter tests.
- [x] Make adapter selection use bundle ID rather than application display name.

Done when a unit test can select an adapter, capture fake state, restore it, and identify the resulting fake window.

## Step 3 — Implement Generic App Recovery

- [x] Capture the bundle ID, application path, and window bounds.
- [x] Relaunch the application through macOS `NSWorkspace`.
- [x] Rely on the application’s native session restoration for app-specific state.
- [x] Store a warning explaining that only generic recovery is available.
- [x] Match relaunched windows using bundle ID first, followed by title and geometry.

Done when a closed generic app can be relaunched and its new window ID can replace the stale saved ID.

## Step 4 — Add `ctx snapshot [workspace]`

- [x] Add the CLI command, defaulting to the active workspace.
- [x] Snapshot only windows assigned to the selected workspace.
- [x] Capture every visible tracked window through its registered adapter.
- [x] Fall back to generic recovery when adapter capture fails.
- [x] Save the fallback warning instead of aborting the entire snapshot.
- [x] Preserve previously captured closed resources when they are not currently visible.
- [x] Write the snapshot atomically.
- [x] Add tests for successful capture, fallback capture, closed-resource preservation, and write failure.

Done when `ctx snapshot coding` safely updates recovery data without losing closed tracked resources.

## Step 5 — Implement the Editor Adapters

### VS Code

- [x] Capture the folder or `.code-workspace` path for each tracked window.
- [x] Restore it with `code --new-window`.
- [x] Match the restored window using bundle ID and project/workspace path before title and geometry.

### Antigravity

- [x] Capture the project folder through Accessibility document metadata.
- [x] Restore the project through the application’s URL handler.
- [x] Match the restored window using bundle ID and project path.

Done when both editors can be closed and restored to the captured project without creating duplicate windows.

## Step 6 — Implement the Warp Adapter

- [x] Capture tracked Warp windows and their tab ordering.
- [x] Capture each tab’s working directory and title when available.
- [x] Never capture or rerun shell commands or processes.
- [x] Restore using Warp URIs and a generated launch configuration.
- [ ] Match all recreated Warp windows before reporting success (switch orchestration).

Done when Warp reopens the saved tabs in the saved directories without executing commands.

## Step 7 — Implement the Firefox Adapter

- [x] Capture every tab URL in each tracked Firefox window through Accessibility/UI automation.
- [x] Preserve tab ordering and the active tab.
- [x] Recreate the Firefox window and its tabs.
- [x] Restore the active tab.
- [x] Record a warning and fall back to generic recovery when tab capture is unavailable.

Done when Firefox restores the URLs, order, and active tab of a closed tracked window.

## Step 8 — Integrate Recovery Into `ctx switch`

- [ ] Reconcile all target workspace windows before launching anything.
- [ ] Recover only windows that are genuinely missing.
- [ ] Launch once per saved resource and avoid duplicate app/window creation.
- [ ] Poll every 250 ms for up to 20 seconds for restored windows.
- [ ] Match restored windows using bundle ID and adapter context before title and geometry.
- [ ] Stage recovery before minimizing the current workspace.
- [ ] If any required target remains missing or ambiguous:
  - Minimize windows created during the failed attempt.
  - Restore focus to the current workspace.
  - Do not change the active workspace in runtime state.
- [ ] When every target resolves:
  - Minimize the previous workspace.
  - Restore the target workspace.
  - Refresh saved window fingerprints and IDs.
  - Persist config and runtime state atomically.
- [ ] Verify repeated switches are idempotent.

Done when a partially failed recovery leaves the original workspace active, while a successful recovery switches cleanly without duplicates.

## Step 9 — Expose Recovery Diagnostics

- [ ] Extend `ctx show` with recovery type, readiness, and last warning.
- [ ] Extend `ctx status` with recovery readiness and warnings.
- [ ] Add the same fields to JSON output.
- [ ] Keep normal human-readable output compact when no warning exists.

Done when users can tell whether every workspace window is recoverable before closing it.

## Step 10 — Automated Verification

- [x] Unit tests:
  - [x] Old YAML loads without recovery fields.
  - [x] Recovery state round-trips through YAML.
  - [x] Reconciliation preserves recovery metadata.
  - [x] Snapshot refreshes visible resources and preserves closed resources.
  - [x] Adapter failure produces a generic fallback and warning.
  - [x] Adapter selection uses bundle ID.
- [ ] Orchestration tests with fake adapters/platform:
  - Existing windows require no launch.
  - Missing windows recover and refresh their IDs.
  - Multiple windows from one app recover without duplication.
  - Partial failure rolls back to the previous active workspace.
  - Repeated recovery is idempotent.
- [x] Run formatting, tests, and strict Clippy for checkpoint `af071d8`.
- [x] Run `graphify update .` after checkpoint `af071d8`.

Done when all automated checks pass.

## Step 11 — End-to-End macOS Smoke Test

- [ ] Create a workspace containing VS Code, Antigravity, Warp, and Firefox.
- [ ] Run `ctx snapshot <workspace>`.
- [ ] Confirm `ctx show <workspace>` reports recovery readiness.
- [ ] Close the tracked windows or applications.
- [ ] Switch away, then run `ctx switch <workspace>`.
- [ ] Verify:
  - Both editor projects reopen.
  - Warp restores its tab directories without running commands.
  - Firefox restores tab URLs, ordering, and the active tab.
  - Window bounds are restored where macOS permits it.
  - No duplicate windows are created on a repeated switch.
- [ ] Force one adapter failure and verify the previous workspace remains active.

Done when the complete recovery flow works on a real Mac and failure rollback is confirmed.

## Later Milestone — macOS Desktop Placement

Do not include Desktop/Space creation or placement in this implementation. After app recovery is stable, add a separate plan for remembering which macOS Desktop contained each window, creating missing Desktops, and moving restored windows to their saved Desktop.

## Fixed Product Decisions

- Snapshots are explicit; they are not automatically taken on every switch.
- Browser URLs are stored as plaintext in the local workspace YAML.
- Closed resources survive later snapshots by default.
- Terminal commands and running processes are never captured or rerun.
- Unsupported apps receive generic bundle-based recovery with a warning.
- A target recovery failure must not tear down the current workspace.
