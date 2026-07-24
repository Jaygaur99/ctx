import type {
  WindowStatus,
  WorkspaceOverview,
  WorkspaceUrlStatus,
} from "../types";

export type BusyAction =
  | { workspace: string; action: "switch" | "open" }
  | { action: "hide_all" }
  | null;

type Tone = "neutral" | "good" | "warning" | "danger" | "accent";

interface CountItem {
  label: string;
  value: number;
  tone: Tone;
}

interface WorkspaceCardProps {
  workspace: WorkspaceOverview;
  busy: BusyAction;
  onSwitch: (name: string) => void;
  onOpenUrls: (name: string) => void;
  onAddWindows: (name: string, trigger: HTMLButtonElement) => void;
  onEdit: (name: string, trigger: HTMLButtonElement) => void;
  showDiagnostics: boolean;
}

function countBy<T extends string>(values: T[], keys: readonly T[]): Record<T, number> {
  return keys.reduce(
    (counts, key) => ({
      ...counts,
      [key]: values.filter((value) => value === key).length,
    }),
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
  const counts = countBy(
    states,
    ["exact", "degraded", "not_ready", "unavailable"] as const,
  );
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

function StateTag({
  children,
  tone,
}: {
  children: React.ReactNode;
  tone: Tone;
}) {
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
      {window.recovery_warning && (
        <p className="warning-text">{window.recovery_warning}</p>
      )}
      {window.placement_warning && (
        <p className="warning-text">{window.placement_warning}</p>
      )}
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

export default function WorkspaceCard({
  workspace,
  busy,
  onSwitch,
  onOpenUrls,
  onAddWindows,
  onEdit,
  showDiagnostics,
}: WorkspaceCardProps) {
  const isBusy = busy?.action !== "hide_all" && busy?.workspace === workspace.name;

  return (
    <article
      className={`workspace-card${workspace.active ? " workspace-card--active" : ""}`}
      aria-label={`${workspace.name} context${workspace.active ? ", active" : ""}`}
    >
      <div className="workspace-card__header">
        <div className="workspace-title">
          <span
            className={`workspace-dot${workspace.active ? " workspace-dot--active" : ""}`}
            aria-hidden="true"
          />
          <div>
            <h2>{workspace.name}</h2>
            {workspace.path && <p title={workspace.path}>{workspace.path}</p>}
          </div>
        </div>
        {workspace.active && <StateTag tone="accent">Active</StateTag>}
      </div>

      {showDiagnostics && (
        <div className="workspace-summary">
          <StatusGroup label="Windows" items={windowCounts(workspace.windows)} />
          <StatusGroup label="Recovery" items={recoveryCounts(workspace.windows)} />
          <StatusGroup label="Placement" items={placementCounts(workspace.windows)} />
          <StatusGroup label="URLs" items={urlCounts(workspace.url_statuses)} />
        </div>
      )}

      <div className="workspace-actions">
        <button
          className="button"
          aria-label={`Edit ${workspace.name} context`}
          disabled={busy !== null}
          onClick={(event) => onEdit(workspace.name, event.currentTarget)}
        >
          Edit
        </button>
        <button
          className="button"
          aria-label={`Add windows to ${workspace.name} context`}
          disabled={busy !== null}
          onClick={(event) => onAddWindows(workspace.name, event.currentTarget)}
        >
          <span aria-hidden="true">＋</span> Add windows
        </button>
        {!workspace.active && (
          <button
            className="button button--primary"
            aria-label={`Switch to ${workspace.name} context`}
            disabled={busy !== null}
            onClick={() => onSwitch(workspace.name)}
          >
            {isBusy && busy.action === "switch" ? "Switching…" : "Switch"}
          </button>
        )}
        {workspace.urls.length > 0 && (
          <button
            className="button"
            aria-label={`Open URLs for ${workspace.name} context`}
            disabled={busy !== null}
            onClick={() => onOpenUrls(workspace.name)}
          >
            {isBusy && busy.action === "open" ? "Opening…" : "Open URLs"}
          </button>
        )}
      </div>

      {showDiagnostics
        && (workspace.windows.length > 0 || workspace.url_statuses.length > 0) && (
        <details className="workspace-details">
          <summary aria-label={`Show ${workspace.name} diagnostics`}>Details</summary>
          {workspace.windows.length > 0 && (
            <section>
              <h3>Tracked windows</h3>
              <ul>
                {workspace.windows.map((window) => (
                  <WindowDetails key={window.saved_id} window={window} />
                ))}
              </ul>
            </section>
          )}
          {workspace.url_statuses.length > 0 && (
            <section>
              <h3>Configured URLs</h3>
              <ul>
                {workspace.url_statuses.map((url) => (
                  <UrlDetails key={url.url} url={url} />
                ))}
              </ul>
            </section>
          )}
        </details>
      )}
    </article>
  );
}
