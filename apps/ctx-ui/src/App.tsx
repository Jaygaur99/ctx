import { useCallback, useEffect, useState } from "react";
import { hidePopover, quitCtx } from "./api";
import {
  ErrorBanner,
  PartialFailureBanner,
  WindowFailureBanner,
} from "./components/AppBanners";
import WorkspaceCard from "./components/WorkspaceCard";
import { useAppearancePreferences } from "./hooks/useAppearancePreferences";
import { useWorkspaceController } from "./hooks/useWorkspaceController";
import ContextEditor from "./sheets/ContextEditor";
import {
  CreateContextSheet,
  DeleteContextSheet,
} from "./sheets/ContextSheets";
import SettingsSheet from "./sheets/SettingsSheet";
import WindowPicker from "./sheets/WindowPicker";
import type {
  AddWindowsReport,
  CreateWorkspaceReport,
  DeleteWorkspacesReport,
} from "./types";

type SheetState =
  | { kind: "create"; returnFocus: HTMLButtonElement }
  | { kind: "delete"; returnFocus: HTMLButtonElement }
  | { kind: "windows"; workspace: string; returnFocus: HTMLButtonElement }
  | { kind: "edit"; workspace: string; returnFocus: HTMLButtonElement }
  | { kind: "settings"; returnFocus: HTMLButtonElement }
  | null;

export default function App() {
  const [sheet, setSheet] = useState<SheetState>(null);
  const {
    overview,
    error,
    partialFailures,
    windowFailures,
    refreshing,
    busy,
    orderedWorkspaces,
    refresh,
    runWorkspaceAction,
    runHideAll,
  } = useWorkspaceController();
  const {
    simpleMode,
    theme,
    setSimpleMode,
    setTheme,
  } = useAppearancePreferences();

  const closeTransientSheet = useCallback(() => {
    const returnFocus = sheet?.returnFocus;
    setSheet(null);
    window.requestAnimationFrame(() => {
      if (returnFocus?.isConnected) returnFocus.focus();
    });
  }, [sheet]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      if (
        sheet?.kind === "edit"
        || sheet?.kind === "settings"
        || sheet?.kind === "windows"
      ) {
        return;
      }
      if (sheet) closeTransientSheet();
      else void hidePopover();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [closeTransientSheet, sheet]);

  const contextCreated = async (report: CreateWorkspaceReport) => {
    if (sheet?.kind !== "create") return;
    const { returnFocus } = sheet;
    await refresh();
    setSheet({
      kind: "windows",
      workspace: report.workspace,
      returnFocus,
    });
  };

  const contextsDeleted = async (_report: DeleteWorkspacesReport) => {
    closeTransientSheet();
    await refresh();
  };

  const windowsAdded = async (_report: AddWindowsReport) => {
    closeTransientSheet();
    await refresh();
  };

  const contextEdited = async () => {
    setSheet(null);
    await refresh();
  };

  const staleActive = overview?.active_workspace
    && !overview.workspaces.some((workspace) => workspace.active);
  const editedWorkspace = sheet?.kind === "edit"
    ? overview?.workspaces.find((workspace) => workspace.name === sheet.workspace)
    : undefined;

  return (
    <main className="app-shell">
      <div
        className="app-base"
        inert={sheet !== null ? true : undefined}
        aria-hidden={sheet !== null ? true : undefined}
      >
        <header className="app-header">
          <div>
            <div className="brand-row">
              <span className="brand-mark" aria-hidden="true">C</span>
              <h1>Ctx</h1>
            </div>
            <p>
              {overview?.active_workspace
                ? `Active: ${overview.active_workspace}`
                : "No active workspace"}
            </p>
          </div>
          <div className="header-actions">
            <button
              className="header-button header-button--icon"
              aria-label="Create context"
              title="Create context"
              disabled={busy !== null}
              onClick={(event) => setSheet({
                kind: "create",
                returnFocus: event.currentTarget,
              })}
            >
              <svg aria-hidden="true" viewBox="0 0 24 24">
                <path d="M12 5v14M5 12h14" />
              </svg>
            </button>
            <button
              className="header-button header-button--icon header-button--danger"
              aria-label="Delete contexts"
              title="Delete contexts"
              disabled={busy !== null || !overview?.workspaces.length}
              onClick={(event) => setSheet({
                kind: "delete",
                returnFocus: event.currentTarget,
              })}
            >
              <svg aria-hidden="true" viewBox="0 0 24 24">
                <path d="M4 7h16M9 7V4h6v3M7 7l1 13h8l1-13M10 11v5M14 11v5" />
              </svg>
            </button>
            <button
              className="header-button header-button--icon"
              aria-label="Settings"
              title="Settings"
              disabled={busy !== null}
              onClick={(event) => setSheet({
                kind: "settings",
                returnFocus: event.currentTarget,
              })}
            >
              <svg aria-hidden="true" viewBox="0 0 24 24">
                <path d="M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7Z" />
                <path
                  d="M19 13.5v-3l-2-.6a7 7 0 0 0-.8-1.8l1-1.9-2.1-2.1-1.9 1a7 7 0 0 0-1.8-.8L10.5 2h-3l-.6 2.1a7 7 0 0 0-1.8.8l-1.9-1-2.1 2.1 1 1.9a7 7 0 0 0-.8 1.8L0 10.5v3l2.1.6a7 7 0 0 0 .8 1.8l-1 1.9L4 19.9l1.9-1a7 7 0 0 0 1.8.8l.6 2.1h3l.6-2.1a7 7 0 0 0 1.8-.8l1.9 1 2.1-2.1-1-1.9a7 7 0 0 0 .8-1.8L19 13.5Z"
                  transform="translate(2.5 0)"
                />
              </svg>
            </button>
            <button
              className="header-button header-button--icon"
              aria-label="Hide all except active context"
              title="Hide all except active context"
              disabled={
                busy !== null
                || !overview?.active_workspace
                || Boolean(staleActive)
              }
              onClick={() => void runHideAll()}
            >
              <svg aria-hidden="true" viewBox="0 0 24 24">
                <path d="M3 3l18 18M10.6 10.7a2 2 0 0 0 2.7 2.7M9.9 4.3A10.6 10.6 0 0 1 12 4c5.5 0 9 5.5 9 5.5a16 16 0 0 1-2.1 2.6M6.6 6.6C4.4 8.1 3 10.5 3 10.5S6.5 16 12 16a9.6 9.6 0 0 0 3.4-.6" />
              </svg>
            </button>
          </div>
        </header>

        <section className="content" aria-live="polite">
          {error && <ErrorBanner error={error} onRetry={() => void refresh()} />}
          {partialFailures.length > 0 && (
            <PartialFailureBanner failures={partialFailures} />
          )}
          {windowFailures.length > 0 && (
            <WindowFailureBanner failures={windowFailures} />
          )}
          {staleActive && (
            <div className="banner banner--warning" role="status">
              <strong>Runtime state is stale</strong>
              <p>
                The active workspace “{overview?.active_workspace}” is no longer in the
                configuration.
              </p>
            </div>
          )}
          {!overview && refreshing && !error && (
            <div className="empty-state">Loading workspaces…</div>
          )}
          {overview && overview.workspaces.length === 0 && (
            <div className="empty-state">
              <strong>No workspaces configured</strong>
              <p>Use Create Context above to add your first context.</p>
              <code>{overview.config_path}</code>
            </div>
          )}
          <div className="workspace-list">
            {orderedWorkspaces.map((workspace) => (
              <WorkspaceCard
                key={workspace.name}
                workspace={workspace}
                busy={busy}
                onSwitch={(name) => void runWorkspaceAction(name, "switch")}
                onOpenUrls={(name) => void runWorkspaceAction(name, "open")}
                onAddWindows={(name, returnFocus) => setSheet({
                  kind: "windows",
                  workspace: name,
                  returnFocus,
                })}
                onEdit={(name, returnFocus) => setSheet({
                  kind: "edit",
                  workspace: name,
                  returnFocus,
                })}
                showDiagnostics={!simpleMode}
              />
            ))}
          </div>
        </section>

        <footer className="app-footer">
          <button
            className="text-button"
            disabled={refreshing || busy !== null}
            onClick={() => void refresh()}
          >
            Refresh
          </button>
          <span>
            {busy?.action === "hide_all"
              ? "Hiding other windows…"
              : busy
                ? `${busy.action === "switch" ? "Switching" : "Opening"} ${busy.workspace}…`
                : "Changes save automatically"}
          </span>
          <button
            className="text-button text-button--danger"
            onClick={() => void quitCtx()}
          >
            Quit
          </button>
        </footer>
      </div>

      {sheet?.kind === "create" && (
        <CreateContextSheet
          onClose={closeTransientSheet}
          onCreated={(report) => void contextCreated(report)}
        />
      )}
      {sheet?.kind === "delete" && overview && (
        <DeleteContextSheet
          workspaces={overview.workspaces}
          activeWorkspace={overview.active_workspace}
          onClose={closeTransientSheet}
          onDeleted={(report) => void contextsDeleted(report)}
        />
      )}
      {sheet?.kind === "windows" && (
        <WindowPicker
          workspace={sheet.workspace}
          onClose={closeTransientSheet}
          onAdded={(report) => void windowsAdded(report)}
        />
      )}
      {sheet?.kind === "edit" && editedWorkspace && (
        <ContextEditor
          workspace={editedWorkspace}
          onClose={() => setSheet(null)}
          onSaved={() => void contextEdited()}
          returnFocus={sheet.returnFocus}
        />
      )}
      {sheet?.kind === "settings" && (
        <SettingsSheet
          onClose={() => setSheet(null)}
          returnFocus={sheet.returnFocus}
          simpleMode={simpleMode}
          theme={theme}
          onSimpleModeChange={setSimpleMode}
          onThemeChange={setTheme}
        />
      )}
    </main>
  );
}
