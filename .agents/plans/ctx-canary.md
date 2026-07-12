# Ctx Canary: Rust CLI Context Switcher

## Goal

Build a local-first, macOS-only Rust CLI that groups existing application windows into workspaces and switches between them by minimizing the active workspace and restoring the target workspace.

Tauri, Git automation, brain-state notes, macOS Spaces, and public distribution are outside the canary.

## Completed

- [x] Create the Cargo workspace with `ctx-core` and `ctx-cli`.
- [x] Add the Clap commands `init`, `list`, `listAll`, `add`, `switch`, `status`, `show`, `remove`, and `close`.
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
- [x] Reconcile stale Core Graphics IDs by application, title, and geometry fingerprints.
- [x] Report saved windows as visible, minimized, ambiguous, or missing.
- [x] Implement exact tracked-window closing with `ctx close [name]`.
- [x] Add `ctx show <name>` for one workspace's live state.
- [x] Add `ctx remove <name>` and clear matching active runtime state.
- [x] Add application/PID filters and assigned-workspace markers to window listings.
- [x] Replace boxed CLI failures with structured errors and consistent exit codes.
- [x] Add global `--json` output for every CLI command and JSON errors.
- [x] Verify a real two-way switch between ChatGPT and VS Code.
- [x] Verify stale-ID refresh from a fake ID to the current Warp window ID.
- [x] Verify add/show/close/remove against a disposable TextEdit window and temporary config.
- [x] Pass all workspace tests and strict Clippy checks.

Completed checkpoint commits:

- `cec8a49 feat: add accessibility window controls`
- `eb3fbc0 feat: persist active workspace state`
- `4d2ac5a feat: implement workspace switching`
- `78193f9 feat: reconcile and manage workspace windows`
- `7e60117 feat: complete workspace lifecycle CLI`

## Completed CLI Work

- [x] Reconcile stale Core Graphics IDs after an application or window restarts.
- [x] Report each saved window as visible, minimized, ambiguous, or missing in `ctx status`.
- [x] Implement `ctx close [name]` for exact tracked windows without quitting unrelated windows.
- [x] Add `ctx show <name>` to inspect one workspace.
- [x] Add `ctx remove <name>` to delete a workspace and clean stale runtime state.
- [x] Improve `list` and `listAll` with application/PID filters and assigned-workspace markers.
- [x] Replace boxed CLI errors with structured error types and consistent exit codes.
- [x] Add optional machine-readable output for future Tauri integration.

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

- `coding` tracks the selected editor and terminal windows.
- `research` tracks the selected research windows.

Core Graphics window IDs are live-session identifiers. Ctx refreshes stale IDs when a unique application/title or application/geometry match exists; otherwise the window is reported as ambiguous or missing.

## Manual Smoke Test

```bash
# Inspect configured workspaces and active state.
cargo run -p ctx-cli -- status

# Show currently visible selectable windows.
cargo run -p ctx-cli -- list

# Show windows across all Desktops, including minimized windows.
cargo run -p ctx-cli -- listAll

# Filter and inspect assignments.
cargo run -p ctx-cli -- listAll --app code
cargo run -p ctx-cli -- show coding

# Machine-readable output.
cargo run -p ctx-cli -- status --json

# Exercise switching in both directions.
cargo run -p ctx-cli -- switch research
cargo run -p ctx-cli -- status
cargo run -p ctx-cli -- switch coding
cargo run -p ctx-cli -- status
```

Expected behavior:

1. Switching to `research` minimizes the saved coding windows and restores the research windows.
2. Switching to `coding` minimizes the research windows and restores/activates the coding windows.
3. `ctx status` reports the most recently selected workspace as active.
4. Windows not assigned to either smoke workspace are left untouched.
