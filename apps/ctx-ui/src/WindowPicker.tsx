import { useCallback, useEffect, useMemo, useState } from "react";
import {
  addWindowsToWorkspace,
  getWindowCandidates,
  getWindowPickerWorkspace,
  hideWindowPicker,
  normalizeCommandError,
  onWindowPickerOpened,
  showPopover,
} from "./api";
import type { CommandError, WindowCandidate, WindowPickerOverview } from "./types";

function CandidateRow({
  candidate,
  checked,
  disabled,
  onToggle,
}: {
  candidate: WindowCandidate;
  checked: boolean;
  disabled: boolean;
  onToggle: (id: number) => void;
}) {
  const assignment = candidate.already_in_workspace
    ? "Already in this workspace"
    : candidate.assigned_to.length > 0
      ? `Also tracked in ${candidate.assigned_to.join(", ")}`
      : "Not tracked yet";

  return (
    <label className={`candidate-row${checked ? " candidate-row--selected" : ""}${disabled ? " candidate-row--disabled" : ""}`}>
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={() => onToggle(candidate.id)}
      />
      <span className="candidate-icon" aria-hidden="true">
        {candidate.application.slice(0, 1).toUpperCase() || "?"}
      </span>
      <span className="candidate-copy">
        <strong title={candidate.title ?? "Untitled window"}>
          {candidate.title ?? "Untitled window"}
        </strong>
        <span>{candidate.application} · PID {candidate.pid}</span>
        <small className={candidate.already_in_workspace ? "candidate-assignment candidate-assignment--tracked" : "candidate-assignment"}>
          {assignment}
        </small>
      </span>
    </label>
  );
}

export default function WindowPicker() {
  const [workspace, setWorkspace] = useState<string | null>(null);
  const [overview, setOverview] = useState<WindowPickerOverview | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [query, setQuery] = useState("");
  const [error, setError] = useState<CommandError | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [saving, setSaving] = useState(false);

  const refresh = useCallback(async (name: string) => {
    setRefreshing(true);
    setError(null);
    try {
      setOverview(await getWindowCandidates(name));
      setSelected(new Set());
    } catch (cause) {
      setOverview(null);
      setError(normalizeCommandError(cause));
    } finally {
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    const open = (name: string) => {
      if (disposed) return;
      setWorkspace(name);
      setQuery("");
      void refresh(name);
    };

    void getWindowPickerWorkspace().then(open).catch(() => undefined);
    void onWindowPickerOpened(open).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refresh]);

  const close = useCallback(async () => {
    await hideWindowPicker().catch(() => undefined);
    await showPopover().catch(() => undefined);
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !saving) void close();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [close, saving]);

  const candidates = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return overview?.windows ?? [];
    return (overview?.windows ?? []).filter((candidate) =>
      [candidate.application, candidate.title ?? "", ...candidate.assigned_to]
        .some((value) => value.toLowerCase().includes(normalized)),
    );
  }, [overview, query]);

  const selectable = candidates.filter((candidate) => !candidate.already_in_workspace);
  const selectedCount = selected.size;
  const addLabel = selectedCount > 0
    ? `Add ${selectedCount} window${selectedCount === 1 ? "" : "s"}`
    : "Add windows";
  const allVisibleSelected = selectable.length > 0
    && selectable.every((candidate) => selected.has(candidate.id));

  const toggle = (id: number) => {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleVisible = () => {
    setSelected((current) => {
      const next = new Set(current);
      for (const candidate of selectable) {
        if (allVisibleSelected) next.delete(candidate.id);
        else next.add(candidate.id);
      }
      return next;
    });
  };

  const addSelected = async () => {
    if (!workspace || selectedCount === 0) return;
    setSaving(true);
    setError(null);
    try {
      await addWindowsToWorkspace(workspace, [...selected]);
      await close();
    } catch (cause) {
      setError(normalizeCommandError(cause));
    } finally {
      setSaving(false);
    }
  };

  return (
    <main className="picker-shell">
      <header className="picker-header">
        <div>
          <div className="brand-row">
            <span className="brand-mark">C</span>
            <h1>Add windows</h1>
          </div>
          <p>{workspace ? `Choose windows for ${workspace}` : "Choose a workspace from the Ctx menu"}</p>
        </div>
        <button className="icon-button icon-button--close" aria-label="Close window picker" disabled={saving} onClick={() => void close()}>×</button>
      </header>

      <div className="picker-toolbar">
        <label className="search-field">
          <span aria-hidden="true">⌕</span>
          <input
            type="search"
            value={query}
            placeholder="Filter by app, title, or workspace"
            aria-label="Filter windows"
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
        <button
          className="icon-button"
          aria-label="Refresh windows"
          disabled={!workspace || refreshing || saving}
          onClick={() => workspace && void refresh(workspace)}
        >
          <span className={refreshing ? "spin" : ""}>↻</span>
        </button>
      </div>

      <section className="picker-content" aria-live="polite">
        {error && (
          <div className={`banner banner--${error.code === "permission" ? "warning" : "danger"}`} role="alert">
            <strong>{error.code === "permission" ? "Permission required" : "Couldn’t update windows"}</strong>
            <p>{error.message}</p>
            {workspace && <button className="text-button" onClick={() => void refresh(workspace)}>Try again</button>}
          </div>
        )}

        <div className="picker-list-heading">
          <span>{refreshing ? "Finding windows…" : `${candidates.length} window${candidates.length === 1 ? "" : "s"}`}</span>
          {selectable.length > 0 && (
            <button className="text-button" disabled={saving} onClick={toggleVisible}>
              {allVisibleSelected ? "Clear visible" : "Select visible"}
            </button>
          )}
        </div>

        {!overview && refreshing && !error && <div className="picker-empty">Looking across all Desktops…</div>}
        {overview && candidates.length === 0 && (
          <div className="picker-empty">
            <strong>{query ? "No matching windows" : "No windows available"}</strong>
            <p>{query ? "Try a different filter." : "Open a window, then refresh the picker."}</p>
          </div>
        )}
        <div className="candidate-list">
          {candidates.map((candidate) => (
            <CandidateRow
              key={candidate.id}
              candidate={candidate}
              checked={selected.has(candidate.id)}
              disabled={saving || candidate.already_in_workspace}
              onToggle={toggle}
            />
          ))}
        </div>
      </section>

      <footer className="picker-footer">
        <div>
          <strong>{selectedCount} selected</strong>
          <span>Desktop placement is captured automatically.</span>
        </div>
        <div className="picker-footer__actions">
          <button className="button" disabled={saving} onClick={() => void close()}>Cancel</button>
          <button className="button button--primary" disabled={!workspace || selectedCount === 0 || saving} onClick={() => void addSelected()}>
            {saving ? "Adding…" : addLabel}
          </button>
        </div>
      </footer>
    </main>
  );
}
