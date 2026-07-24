# Ctx v1.0 — Next Iteration Handoff

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

- [x] Add an explicit Edit action to each context card.
- [x] Present editing as a sheet inside the transient popover, consistent with Create, Delete, and Add Windows.
- [x] Allow renaming the context, removing individual tracked windows, and adding/removing/reordering URLs.
- [x] Keep window discovery in the existing Add Windows picker rather than duplicating it in the editor.
- [x] Confirm destructive window removal and dirty-sheet dismissal where necessary.
- [x] Disable conflicting controls while saving and show core validation or persistence errors inline.
- [x] Refresh from disk after every completed mutation.
- [x] Add keyboard, focus, light/dark, empty, stale, success, and failure coverage.

Done when a user can create, maintain, and switch contexts entirely from the menu bar.

Completed and verified on 2026-07-24. The editor uses the shared core mutation path, explicitly confirms tracked-window removal and dirty dismissal, isolates focus inside the sheet, refreshes after saves, and passes frontend, production-build, Rust workspace, and strict Clippy verification.

## Step 4 — Add Minimal App Settings

- [x] Add a small Settings entry that opens a temporary hidden-by-default panel or sheet; choose the smallest usable presentation after a UI spike.
- [x] Add launch-at-login with an explicit toggle and truthful current-state reporting.
- [x] Show Screen Recording and Accessibility status with links or instructions to the relevant macOS settings.
- [x] Add Open Config Folder, version/build information, and a link to the current GitHub release.
- [x] Keep configuration editing and advanced workspace metadata out of this panel.
- [x] Ensure closing Settings returns Ctx to menu-bar-only behavior with no lingering application window.
- [x] Add tests for setting persistence, launch-at-login failures, permission states, and panel lifecycle.

Done when Ctx can start with the user and explain its own permissions without becoming a conventional desktop app.

Completed and verified on 2026-07-24. Settings stays inside the transient popover, uses the official Tauri LaunchAgent integration, reports permissions without prompting, exposes only fixed help/file destinations, and passes frontend, production-build, Rust workspace, formatting, and strict Clippy verification.
## Step 5 — Add `hideAll` UI Controls

- [x] Add a typed shared-core operation that minimizes every manageable window except windows resolved to the active context.
- [x] Protect every candidate when an active-context window resolves ambiguously, and exclude the Ctx process itself.
- [x] Add a popover control with disabled, success, partial-failure, and error behavior.
- [x] Route the existing CLI command through the same shared operation.

Completed and verified on 2026-07-24. Hide All now preserves only the active context, intentionally includes untracked and ignored windows, keeps the popover closed after a clean run, and reopens it with actionable feedback for skipped windows.

## Step 6 — Add Simple Mode

- [x] Default the context list to Simple Mode.
- [x] Hide the Windows, Recovery, Placement, URL summary, and diagnostic details while keeping context identity, active state, and everyday actions visible.
- [x] Keep Detailed Mode one button away and persist the preference locally.

Completed and verified on 2026-07-24. Simple Mode is the default and its one-click Detailed Mode preference survives future popover sessions.

## Step 7 — Polish, Verify v1.0

- [x] Resolve the highest-value usability findings from Step 0.
- [x] Review popover density, labels, destructive-action language, keyboard navigation, and VoiceOver names.
- [x] Keep the current Ctx icon unless a deliberate brand redesign is requested.
- [x] Run Rust formatting, workspace tests, strict Clippy, and frontend tests/type-check/build.
- [x] Verify the universal Apple Silicon and Intel executable and signed `.app` bundle; leave DMG/Finder packaging with Step 8 per the user.
- [x] Run `git diff --check` and `graphify update .`.
- [x] Update the README for context editing and Settings behavior.

Completed and verified on 2026-07-24. Repeated context actions now have distinct VoiceOver names, every in-popover sheet shares keyboard focus containment, lightweight sheets restore their trigger focus, Simple/Detailed labeling is stable, and the README reflects context editing, Hide All, Simple Mode, Settings, and universal local builds. The full repository-local test and build gate passes; DMG packaging and updater work remain explicitly user-owned.

## Step 8 - Add auto updater using tauri auto update and release it.

- [ ] Bump every package/app version together and publish v1.0 through the tag-only release workflow.

## Step 9 — Verify and Merge into `main`

- [x] Confirm `main` has no unique commits and the merge is a clean fast-forward.
- [x] Run Rust formatting, workspace tests, strict Clippy, frontend tests/type-check/build, and `git diff --check`.
- [x] Refresh the repository knowledge graph.
- [x] Merge the verified `codex/v1-final` history into local `main` without pushing.

Completed and verified on 2026-07-24. The complete v1 implementation history passes the repository-local quality gate and is merged into local `main`. Step 8 remains intentionally pending for the updater, version bump, and release.

The v1.0 rollout is fully done after Step 8 publishes a downloadable artifact through the release workflow.

## Still Deferred

- Git repository automation.
- Service lifecycle controls.
- Pause notes and handoff capture.
- Notifications.
- Broader workspace metadata editing beyond name, windows, and URLs.

Default recommendation: stabilize v0.2 first, then ship context editing and launch-at-login as a focused v0.3 without pulling Git, services, or pause notes into scope.
