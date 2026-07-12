# Ctx Canary: Rust CLI Context Switcher

## Summary

Build a macOS-only Rust CLI proving one workflow:

`ctx switch <name>` stops the current workspace's services, minimizes its tracked VS Code and Chrome windows, restores or opens the target workspace, and starts its services.

No Tauri, Git automation, notes, macOS Spaces, or public npm release.

## Implementation

- Create a Cargo workspace with `ctx-core` and `ctx-cli`; a future Tauri backend can reuse `ctx-core`.
- Use `clap`, `serde`, `serde_yaml`, `serde_json`, `thiserror`, and `directories`.
- Use Rust process groups for background services, redirect output to per-service logs, and stop groups with `SIGTERM`, followed by `SIGKILL` after five seconds.
- Use macOS Accessibility through Rust `AXUIElement` bindings to discover, minimize, restore, raise, and close windows. Accessibility permission is required.
- Add only two application adapters:
  - VS Code: open with `code --new-window <path>` and track the resulting window.
  - Chrome: create a dedicated window for configured URLs and retain its Chrome window ID.
- Never quit VS Code or Chrome; operate only on windows created and tracked by Ctx.
- Persist settings under `~/Library/Application Support/Ctx/config/`, runtime state under `data/`, and logs under `~/Library/Logs/Ctx/`.
- Begin with an automation spike that must successfully open, identify, minimize, restore, and close exact VS Code and Chrome windows before building the complete lifecycle.

## Interfaces

```yaml
version: 1

workspaces:
  devlayout:
    path: /Users/jay/git-work/devLayout
    services:
      - name: web
        run: pnpm dev
      - name: containers
        start: docker compose up -d
        stop: docker compose down
    urls:
      - https://github.com/example/project/pulls
      - http://localhost:3000
```

- `run` defines a long-running process-group service.
- `start` plus `stop` defines a command-managed service such as Docker Compose.
- Commands execute through `/bin/zsh -lc` with the workspace path as the working directory.
- `ctx switch <name>` validates the target, deactivates the current workspace, then activates the target. On activation failure, clean up the partial target and restore the previous workspace.
- `ctx status` reconciles stored PIDs and windows with current system state.
- `ctx close [name]` stops services and closes tracked windows.
- State writes are atomic so interrupted switches remain recoverable.

## Test Plan

- Unit-test YAML validation, state transitions, atomic persistence, and rollback.
- Integration-test process-group start/stop and paired lifecycle commands.
- Add a permission-gated macOS test for exact VS Code and Chrome window control.
- Run an acceptance test with two workspaces:
  - Switching A to B stops A's services and minimizes only A's windows.
  - B's services and windows become active.
  - Switching B to A restores A rather than creating duplicates.
  - Closing A leaves unrelated VS Code and Chrome windows untouched.

## Assumptions

- Personal macOS dogfood only; `ctx` remains a local working command name.
- One workspace is active at a time.
- VS Code's `code` command and Google Chrome are installed.
- Configuration is manually edited for the canary.
- Git, brain-state notes, Tauri UI, generic apps, publishing, and real macOS Desktop/Space control are deferred.
