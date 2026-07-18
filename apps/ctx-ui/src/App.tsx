import { useCallback, useEffect, useMemo, useState } from "react";
import {
  getOverview,
  hidePopover,
  normalizeCommandError,
  onPopoverOpened,
  openWorkspaceUrls,
  quitCtx,
  showPopover,
  showWindowPicker,
  switchWorkspace,
} from "./api";
import type {
  CommandError,
  CtxOverview,
  UrlLaunchFailure,
  WindowStatus,
  WorkspaceOverview,
  WorkspaceUrlStatus,
} from "./types";

type BusyAction = { workspace: string; action: "switch" | "open" } | null;
type Tone = "neutral" | "good" | "warning" | "danger" | "accent";

interface CountItem {
  label: string;
  value: number;
  tone: Tone;
}

function countBy<T extends string>(values: T[], keys: readonly T[]): Record<T, number> {
  return keys.reduce(
    (counts, key) => ({ ...counts, [key]: values.filter((value) => value === key).length }),
    {} as Record<T, number>,
  );
}

function windowCounts(windows: WindowStatus[]): CountItem[] {
  const counts = countBy(
    windows.map((window) => window.state),
    ["visible", "minimized", "missing", "ambiguous"] as const,
  );
  return [
    { label: "Visible", value: counts.visible, tone: "good" },
    { label: "Minimized", value: counts.minimized, tone: "neutral" },
    { label: "Missing", value: counts.missing, tone: "danger" },
    { label: "Ambiguous", value: counts.ambiguous, tone: "warning" },
  ];
}

function recoveryCounts(windows: WindowStatus[]): CountItem[] {
  const states = windows.map((window) => {
    if (!window.recovery_kind) return "unavailable";
    if (!window.recovery_ready) return "not_ready";
    if (window.recovery_degraded) return "degraded";
    return "exact";
  });
  const counts = countBy(states, ["exact", "degraded", "not_ready", "unavailable"] as const);
  return [
    { label: "Exact", value: counts.exact, tone: "good" },
    { label: "Degraded", value: counts.degraded, tone: "warning" },
    { label: "Not ready", value: counts.not_ready, tone: "danger" },
    { label: "Unavailable", value: counts.unavailable, tone: "neutral" },
  ];
}

function placementCounts(windows: WindowStatus[]): CountItem[] {
  const states = windows.map((window) => {
    if (window.placement_degraded) return "degraded";
    return window.placement ? "saved" : "uncaptured";
  });
  const counts = countBy(states, ["saved", "degraded", "uncaptured"] as const);
  return [
    { label: "Saved", value: counts.saved, tone: "good" },
    { label: "Degraded", value: counts.degraded, tone: "warning" },
    { label: "Uncaptured", value: counts.uncaptured, tone: "neutral" },
  ];
}

function urlCounts(urls: WorkspaceUrlStatus[]): CountItem[] {
  const counts = countBy(
    urls.map((url) => url.state),
    ["pending", "opened", "recovery_managed", "failed"] as const,
  );
  return [
    { label: "Pending", value: counts.pending, tone: "neutral" },
    { label: "Opened", value: counts.opened, tone: "good" },
    { label: "Recovery", value: counts.recovery_managed, tone: "accent" },
    { label: "Failed", value: counts.failed, tone: "danger" },
  ];
}

function StatusPill({ label, value, tone = "neutral" }: CountItem) {
  return (
    <span className={`pill pill--${tone}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </span>
  );
}

function StatusGroup({ label, items }: { label: string; items: CountItem[] }) {
  return (
    <div className="status-group">
      <span className="status-group__label">{label}</span>
      <div className="status-group__pills">
        {items.map((item) => (
          <StatusPill key={item.label} {...item} />
        ))}
      </div>
    </div>
  );
}

function StateTag({ children, tone }: { children: React.ReactNode; tone: Tone }) {
  return <span className={`state-tag state-tag--${tone}`}>{children}</span>;
}

function WindowDetails({ window }: { window: WindowStatus }) {
  const recoveryTone: Tone = !window.recovery_kind || !window.recovery_ready
    ? "danger"
    : window.recovery_degraded
      ? "warning"
      : "good";
  const placementTone: Tone = window.placement_degraded
    ? "warning"
    : window.placement
      ? "good"
      : "neutral";
  const windowTone: Tone = window.state === "visible"
    ? "good"
    : window.state === "minimized"
      ? "neutral"
      : window.state === "missing"
        ? "danger"
        : "warning";

  return (
    <li className="detail-item">
      <div className="detail-item__heading">
        <div>
          <strong>{window.title ?? "Untitled window"}</strong>
          <span>{window.owner}</span>
        </div>
        <StateTag tone={windowTone}>{window.state}</StateTag>
      </div>
      <div className="detail-item__tags">
        <StateTag tone={recoveryTone}>
          {window.recovery_kind
            ? `${window.recovery_kind}${window.recovery_ready ? "" : " · not ready"}`
            : "no recovery"}
        </StateTag>
        <StateTag tone={placementTone}>
          {window.placement
            ? `Desktop ${window.placement.desktop_ordinal}`
            : "no placement"}
        </StateTag>
      </div>
      {window.placement && (
        <p className="detail-item__meta" title={window.placement.display_uuid}>
          Display {window.placement.display_uuid}
        </p>
      )}
      {window.recovery_warning && <p className="warning-text">{window.recovery_warning}</p>}
      {window.placement_warning && <p className="warning-text">{window.placement_warning}</p>}
    </li>
  );
}

function UrlDetails({ url }: { url: WorkspaceUrlStatus }) {
  const tone: Tone = url.state === "failed"
    ? "danger"
    : url.state === "pending"
      ? "neutral"
      : url.state === "recovery_managed"
        ? "accent"
        : "good";
  return (
    <li className="detail-item detail-item--url">
      <div className="detail-item__heading">
        <span className="url-text" title={url.url}>{url.url}</span>
        <StateTag tone={tone}>{url.state.replace("_", " ")}</StateTag>
      </div>
      {url.error && <p className="warning-text">{url.error}</p>}
    </li>
  );
}

function WorkspaceCard({
  workspace,
  busy,
  onSwitch,
  onOpenUrls,
  onAddWindows,
}: {
  workspace: WorkspaceOverview;
  busy: BusyAction;
  onSwitch: (name: string) => void;
  onOpenUrls: (name: string) => void;
  onAddWindows: (name: string) => void;
}) {
  const isBusy = busy?.workspace === workspace.name;
  return (
    <article className={`workspace-card${workspace.active ? " workspace-card--active" : ""}`}>
      <div className="workspace-card__header">
        <div className="workspace-title">
          <span className={`workspace-dot${workspace.active ? " workspace-dot--active" : ""}`} />
          <div>
            <h2>{workspace.name}</h2>
            {workspace.path && <p title={workspace.path}>{workspace.path}</p>}
          </div>
        </div>
        {workspace.active && <StateTag tone="accent">Active</StateTag>}
      </div>

      <div className="workspace-summary">
        <StatusGroup label="Windows" items={windowCounts(workspace.windows)} />
        <StatusGroup label="Recovery" items={recoveryCounts(workspace.windows)} />
        <StatusGroup label="Placement" items={placementCounts(workspace.windows)} />
        <StatusGroup label="URLs" items={urlCounts(workspace.url_statuses)} />
      </div>

      <div className="workspace-actions">
        <button
          className="button"
          disabled={busy !== null}
          onClick={() => onAddWindows(workspace.name)}
        >
          <span aria-hidden="true">＋</span> Add windows
        </button>
        {!workspace.active && (
          <button
            className="button button--primary"
            disabled={busy !== null}
            onClick={() => onSwitch(workspace.name)}
          >
            {isBusy && busy.action === "switch" ? "Switching…" : "Switch"}
          </button>
        )}
        {workspace.urls.length > 0 && (
          <button
            className="button"
            disabled={busy !== null}
            onClick={() => onOpenUrls(workspace.name)}
          >
            {isBusy && busy.action === "open" ? "Opening…" : "Open URLs"}
          </button>
        )}
      </div>

      {(workspace.windows.length > 0 || workspace.url_statuses.length > 0) && (
        <details className="workspace-details">
          <summary>Details</summary>
          {workspace.windows.length > 0 && (
            <section>
              <h3>Tracked windows</h3>
              <ul>{workspace.windows.map((window) => <WindowDetails key={window.saved_id} window={window} />)}</ul>
            </section>
          )}
          {workspace.url_statuses.length > 0 && (
            <section>
              <h3>Configured URLs</h3>
              <ul>{workspace.url_statuses.map((url) => <UrlDetails key={url.url} url={url} />)}</ul>
            </section>
          )}
        </details>
      )}
    </article>
  );
}

function ErrorBanner({ error, onRetry }: { error: CommandError; onRetry: () => void }) {
  return (
    <div className={`banner banner--${error.code === "permission" ? "warning" : "danger"}`} role="alert">
      <strong>{error.code === "permission" ? "Permission required" : "Ctx couldn’t refresh"}</strong>
      <p>{error.message}</p>
      <button className="text-button" onClick={onRetry}>Try again</button>
    </div>
  );
}

function PartialFailureBanner({ failures }: { failures: UrlLaunchFailure[] }) {
  return (
    <div className="banner banner--warning" role="status">
      <strong>{failures.length} URL{failures.length === 1 ? "" : "s"} could not be opened</strong>
      {failures.map((failure) => <p key={failure.url}>{failure.url}: {failure.error}</p>)}
    </div>
  );
}

export default function App() {
  const [overview, setOverview] = useState<CtxOverview | null>(null);
  const [error, setError] = useState<CommandError | null>(null);
  const [partialFailures, setPartialFailures] = useState<UrlLaunchFailure[]>([]);
  const [refreshing, setRefreshing] = useState(false);
  const [busy, setBusy] = useState<BusyAction>(null);

  const refresh = useCallback(async () => {
    if (busy) return;
    setRefreshing(true);
    try {
      setOverview(await getOverview());
      setError(null);
    } catch (cause) {
      setError(normalizeCommandError(cause));
    } finally {
      setRefreshing(false);
    }
  }, [busy]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onPopoverOpened(() => void refresh()).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refresh]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") void hidePopover();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const orderedWorkspaces = useMemo(
    () => [...(overview?.workspaces ?? [])].sort((first, second) => {
      if (first.active !== second.active) return first.active ? -1 : 1;
      return first.name.localeCompare(second.name);
    }),
    [overview],
  );

  const runWorkspaceAction = async (workspace: string, action: "switch" | "open") => {
    setBusy({ workspace, action });
    setError(null);
    setPartialFailures([]);
    await hidePopover().catch(() => undefined);
    try {
      const failures = action === "switch"
        ? (await switchWorkspace(workspace)).urls.failed
        : (await openWorkspaceUrls(workspace)).failed;
      setBusy(null);
      await refresh();
      if (failures.length > 0) {
        setPartialFailures(failures);
        await showPopover();
      }
    } catch (cause) {
      setBusy(null);
      setError(normalizeCommandError(cause));
      await showPopover().catch(() => undefined);
    }
  };

  const openWindowPicker = async (workspace: string) => {
    setError(null);
    try {
      await showWindowPicker(workspace);
    } catch (cause) {
      setError(normalizeCommandError(cause));
    }
  };

  const staleActive = overview?.active_workspace && !overview.workspaces.some((workspace) => workspace.active);

  return (
    <main className="app-shell">
      <header className="app-header">
        <div>
          <div className="brand-row">
            <span className="brand-mark">C</span>
            <h1>Ctx</h1>
          </div>
          <p>{overview?.active_workspace ? `Active: ${overview.active_workspace}` : "No active workspace"}</p>
        </div>
        <button className="icon-button" aria-label="Refresh workspaces" disabled={refreshing || busy !== null} onClick={() => void refresh()}>
          <span className={refreshing ? "spin" : ""}>↻</span>
        </button>
      </header>

      <section className="content" aria-live="polite">
        {error && <ErrorBanner error={error} onRetry={() => void refresh()} />}
        {partialFailures.length > 0 && <PartialFailureBanner failures={partialFailures} />}
        {staleActive && (
          <div className="banner banner--warning" role="status">
            <strong>Runtime state is stale</strong>
            <p>The active workspace “{overview?.active_workspace}” is no longer in the configuration.</p>
          </div>
        )}
        {!overview && refreshing && !error && <div className="empty-state">Loading workspaces…</div>}
        {overview && overview.workspaces.length === 0 && (
          <div className="empty-state">
            <strong>No workspaces configured</strong>
            <p>Create one with the Ctx CLI, then refresh this popover.</p>
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
              onAddWindows={(name) => void openWindowPicker(name)}
            />
          ))}
        </div>
      </section>

      <footer className="app-footer">
        <button className="text-button" disabled={refreshing || busy !== null} onClick={() => void refresh()}>Refresh</button>
        <span>{busy ? `${busy.action === "switch" ? "Switching" : "Opening"} ${busy.workspace}…` : "Changes save automatically"}</span>
        <button className="text-button text-button--danger" onClick={() => void quitCtx()}>Quit</button>
      </footer>
    </main>
  );
}
