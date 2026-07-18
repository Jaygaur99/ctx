# Ctx

Ctx is a local-first macOS workspace switcher. Its Rust core tracks application windows, recovers durable app context, restores windows to their saved macOS Desktops, and opens configured workspace URLs.

## Menu-Bar UI

The Tauri UI is a menu-bar utility, not a conventional desktop window. While it is running, Ctx appears only as a status item in the macOS menu bar. Clicking the icon toggles a compact workspace popover; clicking elsewhere or pressing Escape hides it.

The popover shows active, window, recovery, placement, and URL state and provides Switch, Open URLs, and Add windows actions. Add windows opens a temporary picker panel with live windows from every macOS Desktop; selecting windows captures their current Desktop placement and saves them to the chosen workspace. Continue to use the CLI to create workspaces or edit paths, URLs, services, and other workspace metadata.

### Development

Requirements:

- macOS 13 or newer.
- Rust and Cargo.
- Node.js and npm.
- Xcode Command Line Tools.
- Accessibility and Screen Recording permission for the built Ctx app when macOS requests them.

Install the frontend dependencies once:

```bash
cd apps/ctx-ui
npm install
```

Run the menu-bar app in development:

```bash
cd apps/ctx-ui
npm run tauri dev
```

Build a local application bundle:

```bash
cd apps/ctx-ui
npm run tauri build -- --debug
```

The bundle is written beneath the Cargo target directory. Ctx starts with its popover hidden and remains resident until Quit is selected from the popover or the tray icon's context menu.

### Verification

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

cd apps/ctx-ui
npm test
npm run build
```

## License & Ownership

This project is open-sourced under the [MIT License](LICENSE).

**Disclaimer:** This is an independent, personal project by Jay Kumar Gaur. It is completely unaffiliated with and not owned by microsasscapital or any other employer.
