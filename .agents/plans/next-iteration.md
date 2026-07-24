# Ctx v0.3 — Next Iteration Handoff

## Status

Proposed plan for the next working session. We will review the priorities together before implementation and adjust them around real feedback from the v0.2.0 release.

## Where We Are

- Ctx v0.2.0 is published as a universal macOS release for Apple Silicon and Intel.
- Ctx remains a menu-bar utility with no normal main window, Dock icon, or Cmd-Tab presence.
- The popover can create and delete contexts, add windows, show detailed runtime state, switch contexts, force-open context URLs, refresh, and quit.
- The Rust CLI and Tauri UI share `ctx-core`; Tauri contains no workspace business logic.
- Recovery, Desktop placement, URL activation, configuration, and runtime persistence are working through one switch flow.
- Git automation, services, pause notes, and `hideAll` UI remain intentionally deferred.

## Product Boundary

- Keep Ctx menu-bar-first. Do not introduce a persistent conventional application window.
- Keep common actions inside the popover.
- If settings need more room, open a temporary panel only after an explicit Settings action; keep it hidden otherwise.
- Put validation, mutation, persistence, and platform behavior in `ctx-core` and expose thin Tauri commands over typed DTOs.
- Continue loading fresh state for every operation so terminal and UI changes remain visible to each other.

## Step 0 — Start With v0.2 Dogfooding

- [x] Install the published release asset into `/Applications` as a clean-user test.
- [x] Verify first-launch Gatekeeper, Screen Recording, and Accessibility instructions.
- [x] Confirm one menu-bar icon, no Dock icon, no Cmd-Tab entry, and no visible startup window.
- [x] Compare the popover with `ctx status --json` for active context, windows, recovery, placement, and URL state.
- [x] Exercise real switches containing visible, minimized, missing, and recoverable windows across multiple displays and Desktops.
- [x] Verify context creation, individual deletion, delete-all confirmation, window picking, forced URL opening, and external CLI refresh.
- [x] Record concrete bugs or confusing interactions before adding features; fix any data-loss, switching, or recovery issue first.

Done when we trust the release behavior and have a short, prioritized list of real-world friction.

Accepted complete on 2026-07-24. Keep installed-release tests isolated from `tauri dev`: a leftover Vite server or development `ctx-tauri` process can mask `/Applications/Ctx.app` and make permission or launch behavior appear inconsistent.

## Step 1 — Protect Concurrent CLI and UI Mutations

- [x] Add a shared, path-based mutation lock in `ctx-core` so simultaneous CLI and UI writes cannot silently overwrite each other.
- [x] Cover context, window, URL, switch, snapshot, and runtime-marker mutations.
- [x] Use a bounded wait and return a typed busy/lock error with an actionable message.
- [x] Preserve existing atomic transaction and rollback behavior after the lock is acquired.
- [x] Add concurrency tests for competing mutations, lock release after errors, and independent custom config paths.
- [x] Keep read-only overview operations responsive when no inconsistent state can be observed.

Done when Ctx has one cross-process mutation contract rather than relying only on atomic file replacement.

Completed and independently verified on 2026-07-24. Rust workspace tests, strict Clippy, frontend tests, and the production frontend build pass with only this slice applied.

## Step 2 — Complete Context Editing

- [x] Add `ctx-core` APIs and typed reports to rename a context, remove selected windows, and manage its URLs.
- [x] Update active-workspace and URL runtime markers transactionally when a context is renamed.
- [x] Make window removal idempotent and never close the corresponding application window.
- [x] Validate and normalize HTTP/HTTPS URLs in core; support add, remove, and reorder without duplicates.
- [x] Preserve CLI output and exit semantics while routing equivalent commands through the shared facade.
- [x] Add core tests for rename collisions, active rename, stale window removal, URL normalization, ordering, and rollback.

Done when the complete everyday context definition can be maintained without hand-editing YAML.

Completed and independently verified on 2026-07-24. The core-only slice passes the full Rust workspace suite and strict Clippy without exposing the editor in Tauri yet.

## Step 3 — Add an In-Popover Context Editor

- [ ] Add an explicit Edit action to each context card.
- [ ] Present editing as a sheet inside the transient popover, consistent with Create, Delete, and Add Windows.
- [ ] Allow renaming the context, removing individual tracked windows, and adding/removing/reordering URLs.
- [ ] Keep window discovery in the existing Add Windows picker rather than duplicating it in the editor.
- [ ] Confirm destructive window removal and dirty-sheet dismissal where necessary.
- [ ] Disable conflicting controls while saving and show core validation or persistence errors inline.
- [ ] Refresh from disk after every completed mutation.
- [ ] Add keyboard, focus, light/dark, empty, stale, success, and failure coverage.

Done when a user can create, maintain, and switch contexts entirely from the menu bar.

## Step 4 — Add Minimal App Settings

- [ ] Add a small Settings entry that opens a temporary hidden-by-default panel or sheet; choose the smallest usable presentation after a UI spike.
- [ ] Add launch-at-login with an explicit toggle and truthful current-state reporting.
- [ ] Show Screen Recording and Accessibility status with links or instructions to the relevant macOS settings.
- [ ] Add Open Config Folder, version/build information, and a link to the current GitHub release.
- [ ] Keep configuration editing and advanced workspace metadata out of this panel.
- [ ] Ensure closing Settings returns Ctx to menu-bar-only behavior with no lingering application window.
- [ ] Add tests for setting persistence, launch-at-login failures, permission states, and panel lifecycle.

Done when Ctx can start with the user and explain its own permissions without becoming a conventional desktop app.

## Step 5 — Polish, Verify, and Release v0.3

- [ ] Resolve the highest-value usability findings from Step 0.
- [ ] Review popover density, labels, destructive-action language, keyboard navigation, and VoiceOver names.
- [ ] Keep the current Ctx icon unless a deliberate brand redesign is requested.
- [ ] Run Rust formatting, workspace tests, strict Clippy, frontend tests/type-check/build, and a universal Tauri bundle build.
- [ ] Run `git diff --check` and `graphify update .`.
- [ ] Repeat the real macOS acceptance pass on the built artifact.
- [ ] Update the README for context editing and Settings behavior.
- [ ] Bump every package/app version together and publish v0.3.0 through the tag-only release workflow.

Done when v0.3.0 is downloadable, documented, and verified through the same path users receive.

## Still Deferred

- Git repository automation.
- Service lifecycle controls.
- Pause notes and handoff capture.
- Notifications.
- `hideAll` UI controls.
- Auto-update.
- Apple Developer ID signing, notarization, and DMG distribution until credentials and distribution goals are decided.
- Broader workspace metadata editing beyond name, windows, and URLs.
- Non-macOS UI support.

## Tomorrow’s First Conversation

1. What felt wrong or slow while using v0.2.0?
2. Is context editing or launch-at-login more valuable for the next usable checkpoint?
3. Do we want v0.3 to stay a focused daily-use release, or begin signed/notarized distribution work?

Default recommendation: stabilize v0.2 first, then ship context editing and launch-at-login as a focused v0.3 without pulling Git, services, or pause notes into scope.
