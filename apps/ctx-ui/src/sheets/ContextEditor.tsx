import { useCallback, useEffect, useMemo, useState } from "react";
import { editWorkspace, normalizeCommandError } from "../api";
import { trapDialogFocus } from "../dialogFocus";
import type { CommandError, EditWorkspaceReport, WorkspaceOverview } from "../types";

interface UrlRow {
  id: number;
  value: string;
}

export default function ContextEditor({
  workspace,
  onClose,
  onSaved,
  returnFocus,
}: {
  workspace: WorkspaceOverview;
  onClose: () => void;
  onSaved: (report: EditWorkspaceReport) => void;
  returnFocus: HTMLButtonElement | null;
}) {
  const [name, setName] = useState(workspace.name);
  const [urls, setUrls] = useState<UrlRow[]>(
    workspace.urls.map((value, id) => ({ id, value })),
  );
  const [nextUrlId, setNextUrlId] = useState(urls.length);
  const [removedWindowIds, setRemovedWindowIds] = useState<Set<number>>(new Set());
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<CommandError | null>(null);
  const [confirmDiscard, setConfirmDiscard] = useState(false);
  const [confirmRemoval, setConfirmRemoval] = useState(false);

  const submittedUrls = useMemo(
    () => urls.map(({ value }) => value.trim()).filter(Boolean),
    [urls],
  );
  const dirty = useMemo(
    () => name !== workspace.name
      || JSON.stringify(submittedUrls) !== JSON.stringify(workspace.urls)
      || removedWindowIds.size > 0,
    [name, submittedUrls, workspace.name, workspace.urls, removedWindowIds],
  );

  const requestClose = useCallback(() => {
    if (saving) return;
    if (dirty && !confirmDiscard) {
      setConfirmDiscard(true);
      return;
    }
    onClose();
  }, [confirmDiscard, dirty, onClose, saving]);

  useEffect(() => () => {
    if (returnFocus?.isConnected) returnFocus.focus();
  }, [returnFocus]);

  const resetConfirmations = () => {
    setConfirmDiscard(false);
    setConfirmRemoval(false);
  };

  const handleDialogKeyDown = (event: React.KeyboardEvent<HTMLElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      requestClose();
      return;
    }
    trapDialogFocus(event);
  };

  const updateUrl = (id: number, value: string) => {
    resetConfirmations();
    setUrls((current) => current.map((row) => row.id === id ? { ...row, value } : row));
  };

  const moveUrl = (index: number, direction: -1 | 1) => {
    resetConfirmations();
    setUrls((current) => {
      const target = index + direction;
      if (target < 0 || target >= current.length) return current;
      const reordered = [...current];
      [reordered[index], reordered[target]] = [reordered[target], reordered[index]];
      return reordered;
    });
  };

  const addUrl = () => {
    resetConfirmations();
    setUrls((current) => [...current, { id: nextUrlId, value: "" }]);
    setNextUrlId((current) => current + 1);
  };

  const toggleWindowRemoval = (id: number) => {
    resetConfirmations();
    setRemovedWindowIds((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const save = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!name.trim() || !dirty) return;
    if (removedWindowIds.size > 0 && !confirmRemoval) {
      setConfirmDiscard(false);
      setConfirmRemoval(true);
      return;
    }
    setSaving(true);
    setError(null);
    try {
      onSaved(await editWorkspace(
        workspace.name,
        name.trim(),
        submittedUrls,
        [...removedWindowIds],
      ));
    } catch (cause) {
      setError(normalizeCommandError(cause));
    } finally {
      setSaving(false);
    }
  };

  return (
    <section
      className="sheet sheet--compact"
      role="dialog"
      aria-modal="true"
      aria-labelledby="edit-context-title"
      onKeyDown={handleDialogKeyDown}
    >
      <header className="sheet-header">
        <div>
          <h2 id="edit-context-title">Edit context</h2>
          <p>Rename it, maintain URLs, or stop tracking windows.</p>
        </div>
        <button className="icon-button icon-button--close" aria-label="Close context editor" disabled={saving} onClick={requestClose}>×</button>
      </header>
      <form className="sheet-form" onSubmit={(event) => void save(event)}>
        {error && <div className="banner banner--danger" role="alert"><strong>Couldn’t save context</strong><p>{error.message}</p></div>}
        {confirmDiscard && (
          <div className="banner banner--warning" role="alert">
            <strong>Discard unsaved changes?</strong>
            <p>Click Close again to discard them, or keep editing.</p>
          </div>
        )}
        {confirmRemoval && (
          <div className="banner banner--warning" role="alert">
            <strong>
              Remove {removedWindowIds.size} tracked window{removedWindowIds.size === 1 ? "" : "s"}?
            </strong>
            <p>
              This only stops tracking the selected window{removedWindowIds.size === 1 ? "" : "s"}; it never closes an app. Confirm to save.
            </p>
          </div>
        )}

        <label className="field-label" htmlFor="edit-context-name">Context name</label>
        <input id="edit-context-name" className="text-field" value={name} disabled={saving} autoFocus onChange={(event) => { resetConfirmations(); setName(event.target.value); }} />

        <div className="editor-heading">
          <div><strong>URLs</strong><span>Opened during context switches, in this order.</span></div>
          <button type="button" className="button" disabled={saving} onClick={addUrl}>＋ Add URL</button>
        </div>
        <div className="editor-list">
          {urls.length === 0 && <p className="editor-empty">No URLs configured.</p>}
          {urls.map((row, index) => (
            <div className="url-editor-row" key={row.id}>
              <input className="text-field" aria-label={`URL ${index + 1}`} value={row.value} disabled={saving} placeholder="https://example.com" onChange={(event) => updateUrl(row.id, event.target.value)} />
              <div className="row-buttons">
                <button type="button" className="mini-button" aria-label={`Move URL ${index + 1} up`} disabled={saving || index === 0} onClick={() => moveUrl(index, -1)}>↑</button>
                <button type="button" className="mini-button" aria-label={`Move URL ${index + 1} down`} disabled={saving || index === urls.length - 1} onClick={() => moveUrl(index, 1)}>↓</button>
                <button type="button" className="mini-button mini-button--danger" aria-label={`Remove URL ${index + 1}`} disabled={saving} onClick={() => { resetConfirmations(); setUrls((current) => current.filter((candidate) => candidate.id !== row.id)); }}>×</button>
              </div>
            </div>
          ))}
        </div>

        <div className="editor-heading">
          <div><strong>Tracked windows</strong><span>Removal only forgets the window; it never closes the app.</span></div>
        </div>
        <div className="editor-list">
          {workspace.windows.length === 0 && <p className="editor-empty">No windows tracked.</p>}
          {workspace.windows.map((window) => {
            const removing = removedWindowIds.has(window.saved_id);
            return (
              <div className={`window-editor-row${removing ? " window-editor-row--removing" : ""}`} key={window.saved_id}>
                <div><strong>{window.title ?? "Untitled window"}</strong><span>{window.owner} · {window.state}</span></div>
                <button
                  type="button"
                  className={`button${removing ? " button--danger" : ""}`}
                  aria-pressed={removing}
                  disabled={saving}
                  onClick={() => toggleWindowRemoval(window.saved_id)}
                >
                  {removing ? "Keep" : "Remove"}
                </button>
              </div>
            );
          })}
        </div>

        <div className="sheet-actions">
          <button type="button" className="button" disabled={saving} onClick={requestClose}>{confirmDiscard ? "Discard changes" : "Cancel"}</button>
          <button type="submit" className="button button--primary" disabled={saving || !name.trim() || !dirty}>
            {saving ? "Saving…" : confirmRemoval ? "Confirm removal & save" : "Save context"}
          </button>
        </div>
      </form>
    </section>
  );
}
