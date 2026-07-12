# Ctx Canary: Rust CLI Context Switcher

## Goal

Build a local-first, macOS-only Rust CLI that groups existing application windows into workspaces and switches between them by minimizing the active workspace and restoring the target workspace.

Tauri, Git automation, brain-state notes, macOS Spaces, and public distribution are outside the canary.

## Completed

- [x] Create the Cargo workspace with `ctx-core` and `ctx-cli`.
- [x] Add the Clap commands `init`, `list`, `listAll`, `add`, `switch`, `status`, and the `close` placeholder.
- [x] Resolve standard macOS config, data, and log paths.
- [x] Create and validate `workspaces.yaml` with typed errors.
- [x] List visible windows with Core Graphics.
- [x] List minimized/off-screen windows across all macOS Desktops with `listAll`.
- [x] Save selected window IDs, PIDs, owners, titles, and geometry with `ctx add`.
- [x] Detect and request macOS Accessibility permission.
- [x] Resolve saved windows to Accessibility elements using title and geometry fingerprints.
- [x] Support Electron applications such as VS Code by activating them before restore.
- [x] Minimize and restore individual windows without hiding or quitting the owning application.
- [x] Persist the active workspace atomically in `runtime.json`.
- [x] Implement `ctx switch <name>` with an attempt to restore the previous workspace if target activation fails.
- [x] Report the active workspace through `ctx status`.
- [x] Verify a real two-way switch between ChatGPT and VS Code.
- [x] Pass all workspace tests and strict Clippy checks.

Completed checkpoint commits:

- `cec8a49 feat: add accessibility window controls`
- `eb3fbc0 feat: persist active workspace state`
- `4d2ac5a feat: implement workspace switching`

## Remaining CLI Work

- [ ] Reconcile stale Core Graphics IDs after an application or window restarts.
- [ ] Report each saved window as visible, minimized, ambiguous, or missing in `ctx status`.
- [ ] Implement `ctx close [name]` for exact tracked windows without quitting unrelated windows.
- [ ] Add `ctx show <name>` to inspect one workspace.
- [ ] Add `ctx remove <name>` to delete a workspace and clean stale runtime state.
- [ ] Improve `list` and `listAll` with application/PID filters and assigned-workspace markers.
- [ ] Replace boxed CLI errors with structured error types and consistent exit codes.
- [ ] Add optional machine-readable output for future Tauri integration.

## Deferred Service Work

- [ ] Launch `run` services through `/bin/zsh -lc` in isolated process groups.
- [ ] Redirect service output to per-workspace log files.
- [ ] Stop process groups with `SIGTERM`, then `SIGKILL` after a timeout.
- [ ] Support paired `start`/`stop` commands such as Docker Compose.
- [ ] Persist PID/PGID service state and reconcile stale processes.
- [ ] Integrate service stop/start and rollback into `ctx switch`.
- [ ] Add `ctx logs <workspace> [service]`.

## Deferred Recovery and Product Work

- [ ] Add a VS Code adapter that recreates missing project windows with `code --new-window`.
- [ ] Add a Chrome adapter that recreates missing URL windows.
- [ ] Add permission-gated macOS integration tests for minimize, restore, switch, and close.
- [ ] Add process lifecycle integration tests.
- [ ] Document installation, Accessibility setup, usage, and current limitations.
- [ ] Run final acceptance testing without leaving temporary workspace definitions behind.

## Current Configuration

Persistent files:

- Config: `~/Library/Application Support/Ctx/config/workspaces.yaml`
- Runtime: `~/Library/Application Support/Ctx/data/runtime.json`
- Logs: `~/Library/Logs/Ctx/`

Current smoke workspaces:

- `smoke-chat` tracks the ChatGPT window.
- `smoke-code` tracks the VS Code window.

Core Graphics window IDs are live-session identifiers. The current canary expects tracked windows to remain open; restarting an application can make its saved ID stale.

## Manual Smoke Test

```bash
# Inspect configured workspaces and active state.
cargo run -p ctx-cli -- status

# Show currently visible selectable windows.
cargo run -p ctx-cli -- list

# Show windows across all Desktops, including minimized windows.
cargo run -p ctx-cli -- listAll

# Exercise switching in both directions.
cargo run -p ctx-cli -- switch smoke-chat
cargo run -p ctx-cli -- status
cargo run -p ctx-cli -- switch smoke-code
cargo run -p ctx-cli -- status
```

Expected behavior:

1. Switching to `smoke-chat` minimizes the saved VS Code window and restores ChatGPT.
2. Switching to `smoke-code` minimizes ChatGPT and restores/activates VS Code.
3. `ctx status` reports the most recently selected workspace as active.
4. Windows not assigned to either smoke workspace are left untouched.
