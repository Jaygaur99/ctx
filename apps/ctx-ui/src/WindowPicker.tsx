import { useCallback, useEffect, useMemo, useState } from "react";
import { addWindowsToWorkspace, getWindowCandidates, normalizeCommandError } from "./api";
import { trapDialogFocus } from "./dialogFocus";
import type {
  AddWindowsReport,
  CommandError,
  WindowCandidate,
  WindowPickerOverview,
} from "./types";

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
    ? "Already in this context"
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

export default function WindowPicker({
  workspace,
  onClose,
  onAdded,
}: {
  workspace: string;
  onClose: () => void;
  onAdded: (report: AddWindowsReport) => void;
}) {
  const [overview, setOverview] = useState<WindowPickerOverview | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [query, setQuery] = useState("");
  const [error, setError] = useState<CommandError | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [saving, setSaving] = useState(false);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    setError(null);
    try {
      setOverview(await getWindowCandidates(workspace));
      setSelected(new Set());
    } catch (cause) {
      setOverview(null);
      setError(normalizeCommandError(cause));
    } finally {
      setRefreshing(false);
    }
  }, [workspace]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !saving) onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, saving]);

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
    if (selectedCount === 0) return;
    setSaving(true);
    setError(null);
    try {
      onAdded(await addWindowsToWorkspace(workspace, [...selected]));
    } catch (cause) {
      setError(normalizeCommandError(cause));
    } finally {
      setSaving(false);
    }
  };

  return (
    <section
      className="sheet"
      role="dialog"
      aria-modal="true"
      aria-labelledby="window-picker-title"
      onKeyDown={trapDialogFocus}
    >
      <header className="sheet-header">
        <div>
          <h2 id="window-picker-title">Add windows</h2>
          <p>Choose windows for {workspace}</p>
        </div>
        <button className="icon-button icon-button--close" aria-label="Close window picker" disabled={saving} onClick={onClose}>×</button>
      </header>

      <div className="picker-toolbar">
        <label className="search-field">
          <span aria-hidden="true">⌕</span>
          <input
            type="search"
            value={query}
            placeholder="Filter by app, title, or context"
            aria-label="Filter windows"
            autoFocus
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
        <button className="icon-button" aria-label="Refresh windows" disabled={refreshing || saving} onClick={() => void refresh()}>
          <span className={refreshing ? "spin" : ""}>↻</span>
        </button>
      </div>

      <div className="picker-content" aria-live="polite">
        {error && (
          <div className={`banner banner--${error.code === "permission" ? "warning" : "danger"}`} role="alert">
            <strong>{error.code === "permission" ? "Permission required" : "Couldn’t update windows"}</strong>
            <p>{error.message}</p>
            <button className="text-button" onClick={() => void refresh()}>Try again</button>
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
      </div>

      <footer className="picker-footer">
        <div>
          <strong>{selectedCount} selected</strong>
          <span>Desktop placement is captured automatically.</span>
        </div>
        <div className="picker-footer__actions">
          <button className="button" disabled={saving} onClick={onClose}>Cancel</button>
          <button className="button button--primary" disabled={selectedCount === 0 || saving} onClick={() => void addSelected()}>
            {saving ? "Adding…" : addLabel}
          </button>
        </div>
      </footer>
    </section>
  );
}
