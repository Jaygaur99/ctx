# Ctx

<p align="center">
  <img src="apps/ctx-ui/src-tauri/icons/128x128.png" width="96" alt="Ctx app icon">
</p>

<p align="center">
  <strong>Switch your whole macOS workspace from the menu bar.</strong>
</p>

<p align="center">
  <a href="https://github.com/Jaygaur99/ctx/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/Jaygaur99/ctx"></a>
  <a href="https://github.com/Jaygaur99/ctx/actions/workflows/release.yml"><img alt="Release build" src="https://github.com/Jaygaur99/ctx/actions/workflows/release.yml/badge.svg"></a>
  <img alt="macOS 13+" src="https://img.shields.io/badge/macOS-13%2B-black?logo=apple">
  <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-blue.svg"></a>
</p>

Ctx is a local-first workspace switcher for macOS. A context is a group of application windows and URLs that belong together: your editor, terminal, browser, documentation, and anything else needed for a project. Select a context and Ctx recovers missing app state, restores windows to their saved macOS Desktops, opens its URLs, and brings the workspace forward.

The app lives entirely in the menu bar. It has no normal main window, Dock icon, or Cmd-Tab entry.

> Ctx is an early release. Back up `~/Library/Application Support/Ctx` before relying on it for important workspace state.

## What it does

- **Menu-bar first.** Click the status item for a compact popover; click outside it or press Escape to hide it.
- **One-action switching.** Recover missing windows, restore Desktop placement, open configured URLs, and activate the target context through the same `ctx-core` flow used by the CLI.
- **Complete context editing in the UI.** Create, rename, or delete contexts; add or forget tracked windows; and add, remove, or reorder URLs without opening a conventional app window.
- **Simple by default, detailed on demand.** Everyday actions stay compact while Detailed View exposes visible, minimized, missing, or ambiguous windows plus recovery, placement, and URL status.
- **Hide everything else.** One menu-bar action minimizes every manageable window except the windows in the active context.
- **Built-in settings.** Start Ctx at login, review Screen Recording and Accessibility status, open the configuration folder, and inspect the installed version.
- **Durable app recovery.** Capture richer state for VS Code, Antigravity, Warp, and Firefox; other apps use a generic relaunch fallback.
- **CLI and JSON output.** Inspect and control the same workspaces from scripts and the terminal.
- **Local-first storage.** Configuration and runtime markers stay on your Mac. Ctx has no account or hosted service.

## Requirements

- macOS 13 Ventura or newer.
- **Screen Recording** permission to discover windows across macOS Desktops.
- **Accessibility** permission to minimize, focus, and move windows.

macOS prompts for these permissions when they are first needed. You can review them in **System Settings → Privacy & Security**. If a permission change does not take effect immediately, quit and reopen Ctx.

## Install the menu-bar app

1. Download the macOS `.app.tar.gz` asset from the [latest GitHub Release](https://github.com/Jaygaur99/ctx/releases/latest).
2. Extract it and move `Ctx.app` into `/Applications`.
3. Open Ctx. Its icon appears in the menu bar; no normal window opens.
4. Grant Screen Recording and Accessibility access when macOS asks.

Release builds are ad-hoc signed but are not yet Apple-notarized. On first launch, macOS may require you to right-click **Ctx.app**, choose **Open**, and confirm, or allow it under **System Settings → Privacy & Security**.

## Quick start

1. Click the Ctx menu-bar icon.
2. Choose **Create context**, enter a name, and create it.
3. In the window picker, select the windows that belong to the context and choose **Add windows**.
4. Choose **Edit** on a context to rename it, maintain its URLs, or stop tracking selected windows.
5. Create another context, then use **Switch** to move between them.
6. Use **Hide all except active context** to minimize everything outside the active context.
7. Toggle **Detailed View** when you need window, recovery, Desktop placement, or URL diagnostics.

**Open URLs** always opens every configured URL for that context without changing the active context.

## CLI

Install the CLI from a clone of this repository:

```bash
cargo install --path crates/ctx-cli
ctx init
```

A small terminal workflow looks like this:

```bash
# Discover windows on every Desktop, including minimized windows
ctx listAll

# Create a context from listed window IDs
ctx add coding 123 456

# Save recoverable app state and current Desktop placement
ctx snapshot coding

# Associate URLs and switch into the context
ctx url add coding https://github.com https://docs.rs
ctx switch coding

# Inspect the complete state
ctx status
ctx status --json
```

### Command overview

| Command | Purpose |
| --- | --- |
| `ctx init` | Create the default configuration file. |
| `ctx list` | List currently visible windows. |
| `ctx listAll` | List selectable windows across all Desktops. |
| `ctx spaces [WINDOW_ID]` | Inspect displays, Desktops, or one window's placement. |
| `ctx add NAME WINDOW_ID...` | Create a context from live windows. |
| `ctx snapshot [NAME]` | Refresh recovery and placement state. |
| `ctx switch NAME` | Run the complete context switch flow. |
| `ctx show NAME` | Show one context and its live state. |
| `ctx status` | Show all contexts and the active context. |
| `ctx url add\|remove\|list\|open` | Manage and launch context URLs. |
| `ctx ignore` / `ctx unignore` | Mark or unmark windows as ignored in status and window-assignment views. |
| `ctx hideAll` | Minimize windows outside the active context. |
| `ctx close [NAME]` | Stop and close a context. |
| `ctx remove NAME` | Delete a context definition. |

Use `ctx help`, `ctx help <COMMAND>`, or the global `--json` flag for the full interface. Pass `--config PATH` to use a different workspace configuration file.

## Configuration and state

Ctx stores its files in the standard per-user macOS locations:

| Data | Default path |
| --- | --- |
| Workspace configuration | `~/Library/Application Support/Ctx/config/workspaces.yaml` |
| Runtime state | `~/Library/Application Support/Ctx/data/runtime.json` |
| Logs directory | `~/Library/Logs/Ctx` |

The UI manages context names, tracked windows, and URLs. Advanced metadata can be edited with the CLI or directly in `workspaces.yaml` while Ctx is not performing an action. A minimal configuration is:

```yaml
version: 1
workspaces:
  coding:
    path: /Users/you/code/project
    urls:
      - https://github.com
      - https://docs.rs
    windows: []
```

The runtime file is managed by Ctx and should not be edited by hand.

## Recovery support

| Application | Captured state |
| --- | --- |
| Visual Studio Code, Insiders, Code OSS | Project or workspace path |
| Antigravity | Project path |
| Warp | Tabs, working directories, and active tab |
| Firefox | Tabs, URLs, and active tab |
| Other macOS applications | Generic app relaunch with best-effort window matching |

Recovery is deliberately best-effort. The popover and `ctx status` surface degraded or unavailable state instead of pretending a window can be restored exactly.

## Architecture

Ctx is a Rust workspace with three layers:

- `crates/ctx-core` owns discovery, recovery, Desktop placement, URL launching, switching, configuration, and persistence.
- `crates/ctx-cli` is a terminal interface over the core behavior.
- `apps/ctx-ui` is a thin Tauri v2 and React menu-bar interface that calls the same core APIs.

Business logic belongs in `ctx-core`; the Tauri commands should remain a small IPC boundary.

## Develop

You need Rust, Node.js 22+, npm, and the Xcode Command Line Tools.

```bash
git clone https://github.com/Jaygaur99/ctx.git
cd ctx/apps/ctx-ui
npm ci
npm run tauri dev
```

Useful verification commands:

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

cd apps/ctx-ui
npm test
npm run build
```

Build a local application bundle with:

```bash
cd apps/ctx-ui
npm run tauri build
```

Build the same universal Apple Silicon and Intel bundle used by releases with:

```bash
cd apps/ctx-ui
npm run tauri build -- --target universal-apple-darwin
```

## Releases

The release workflow runs **only** when a version tag matching `v*` is pushed. It builds a universal macOS app for Apple Silicon and Intel, creates a GitHub Release, attaches the app bundle, and generates release notes.

```bash
git tag v0.2.0
git push origin v0.2.0
```

Branch pushes and pull requests do not run the release workflow.

## Current scope

Ctx currently supports macOS only. Auto-update, notifications, Developer ID signing/notarization, Git automation, services UI, and pause notes are planned for later iterations.

Issues and focused pull requests are welcome. Please keep platform behavior and persistence logic in `ctx-core` so the CLI and menu-bar app cannot drift apart.

## License and ownership

Ctx is available under the [MIT License](LICENSE).

This is an independent personal project by Jay Kumar Gaur. It is unaffiliated with and not owned by microsasscapital or any other employer.

It was designed and built with extensive use of AI-assisted development. Product direction, architectural decisions, review, testing, and releases are the author's responsibility.
