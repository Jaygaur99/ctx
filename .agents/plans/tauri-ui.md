# Ctx Tauri Menu-Bar UI — Step-by-Step Plan

## Summary

Build a macOS-only Tauri v2 menu-bar utility. Ctx has no conventional application window or Dock presence: clicking its status-item icon toggles a compact popover, while future window-picker and settings interfaces may use temporary hidden panels.

The UI depends directly on `ctx-core`; it never invokes or parses the CLI. React, TypeScript, Vite, and npm provide the popover frontend.

## Fixed Product Decisions

- [x] The first milestone contains the workspace overview, refresh, switch, open-URLs, and quit actions only.
- [x] Window picking, settings, and launch-at-login are deferred.
- [x] “Open URLs” explicitly reopens all configured URLs for that workspace (`force = true`) without changing the active workspace.
- [x] Switching uses the existing complete core flow: recovery, desktop placement, URL activation, window switching, and transactional persistence.
- [x] The popover refreshes whenever it opens, after actions, and on manual request; it does not poll while open.
- [x] Git automation, services, pause notes, and configuration editing remain deferred.
- [x] `hideAll` remains available through the core/CLI but is not exposed in this first UI.

## Step 0 — Prepare the Baseline

- [x] Create the implementation branch from the current work and merge the current `origin/main`, which contains recovery, placement, and URL support.
- [x] Preserve the unrelated dirty changes in `.agents/plans/app-recovery.md` byte-for-byte; do not stash, rewrite, or include them in milestone changes.
- [x] Run the existing workspace tests and strict Clippy checks to establish a clean baseline.
- [x] Confirm Node/npm, Rust, and the macOS command-line build tools can scaffold and compile a minimal Tauri application.

## Step 1 — Add a Shared Core Application API

- [x] Add a path-based `ctx_core::CtxApp` facade with production discovery and test constructors.
- [x] Add serializable `CtxOverview`, `WorkspaceOverview`, and typed `CtxAppError` public types while reusing the existing window, URL, switch, and launch-report types.
- [x] Implement `CtxApp::overview()` by loading config/runtime state once, discovering live windows once, obtaining the boot ID once, and assembling every workspace’s window, recovery, placement, and URL state.
- [x] Implement `CtxApp::switch_workspace(name)` using the existing switch orchestration and coordinated config/runtime persistence.
- [x] Implement `CtxApp::open_workspace_urls(name, force)` using the existing launcher and persist runtime markers even when some URLs fail.
- [x] Refactor CLI `status`, `show`, `switch`, and `url open` paths to use this facade while preserving existing output and exit behavior.
- [x] Keep status assembly, validation, persistence, URL semantics, and workspace behavior out of Tauri command handlers.

## Step 2 — Scaffold the Menu-Bar Application

- [x] Add `apps/ctx-ui` with a Tauri Rust crate, React/TypeScript/Vite frontend, npm lockfile, and membership in the Cargo workspace.
- [x] Pin Tauri dependencies to one v2 minor release and use built-in tray/window APIs.
- [x] Configure one hidden, borderless, fixed 400×560 `popover` webview with a shadow and no conventional window behavior.
- [x] Set macOS activation policy to `Accessory` so Ctx has no Dock or application-switcher presence.
- [x] Add a monochrome template status icon, active-workspace tooltip, and right-click Quit menu.
- [x] Toggle and position the popover below the tray icon, clamped to the current monitor.
- [x] Hide on Escape, focus loss, a second tray click, or successful action completion; remain resident until explicit Quit.

## Step 3 — Implement the Thin Tauri Boundary

- [x] Expose only `get_overview`, `switch_workspace`, `open_workspace_urls`, `hide_popover`, `show_popover`, and `quit` commands.
- [x] Run core work off the UI thread and serialize Tauri-originated operations behind one gate.
- [x] Load config/runtime from disk for every command.
- [x] Return core DTOs unchanged and serialize errors as `{ code, message }`.
- [x] Emit `ctx://popover-opened` every time the tray reveals the popover.
- [x] Hide before external app launches; remain hidden on success and reopen with errors or partial failures.

## Step 4 — Build the Workspace Popover

- [x] Add a compact header with Ctx, active workspace, refresh progress, and manual refresh.
- [x] Render the active workspace first and the remaining workspaces alphabetically.
- [x] Summarize window, recovery, placement, and URL states for each workspace.
- [x] Add expandable per-window and per-URL details with warnings.
- [x] Show Switch only for inactive workspaces and Open URLs only where URLs are configured.
- [x] Disable mutations while an action runs and identify the in-progress action.
- [x] Provide loading, empty, stale-active, permission, general-error, and URL partial-failure states.
- [x] Add an accessible footer with Refresh and Quit and support macOS light/dark appearance.

## Step 5 — Automated Verification

- [x] Add core overview/action tests and CLI regression tests.
- [x] Add Rust tray-position, toggle, serialization, and command-gate tests.
- [x] Add frontend rendering and interaction tests.
- [x] Run Rust formatting, tests, strict Clippy, frontend type-check/tests/build, a debug Tauri bundle build, `git diff --check`, and `graphify update .`.

## Step 6 — macOS Acceptance Test and Documentation

- [x] Document development, build, permission, and launch instructions in the README.
- [x] Verify menu-bar-only launch, tray interactions, dismissal, and positioning logic.
- [ ] Compare popover state against `ctx status --json`.
- [ ] Exercise real workspace switching, recovery, placement, URL opening, errors, and external CLI refresh.
- [x] Confirm no picker/settings panel or conventional main window is exposed.

## Assumptions and Deferred Work

- macOS is the only supported UI platform.
- The popover is technically a hidden webview window but behaves exclusively as a transient status-item popover.
- Cross-process file locking is not added; existing atomic persistence remains in effect.
- Window picker, workspace editing, settings, launch-at-login, `hideAll` controls, Git automation, services, pause notes, notifications, auto-update, and distribution/signing are later milestones.
- Future picker/settings interfaces must open as temporary panels and remain hidden unless explicitly requested.
