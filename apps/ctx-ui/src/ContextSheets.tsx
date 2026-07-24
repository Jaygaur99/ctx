import { useState } from "react";
import {
  createWorkspace,
  deleteAllWorkspaces,
  deleteWorkspace,
  normalizeCommandError,
} from "./api";
import { trapDialogFocus } from "./dialogFocus";
import type {
  CommandError,
  CreateWorkspaceReport,
  DeleteWorkspacesReport,
  WorkspaceOverview,
} from "./types";

function SheetError({ error }: { error: CommandError }) {
  return (
    <div className="banner banner--danger" role="alert">
      <strong>Couldn’t save the change</strong>
      <p>{error.message}</p>
    </div>
  );
}

export function CreateContextSheet({
  onClose,
  onCreated,
}: {
  onClose: () => void;
  onCreated: (report: CreateWorkspaceReport) => void;
}) {
  const [name, setName] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<CommandError | null>(null);
  const normalizedName = name.trim();

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!normalizedName) return;
    setSaving(true);
    setError(null);
    try {
      onCreated(await createWorkspace(normalizedName));
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
      aria-labelledby="create-context-title"
      onKeyDown={trapDialogFocus}
    >
      <header className="sheet-header">
        <div>
          <h2 id="create-context-title">Create context</h2>
          <p>Create an empty context, then choose its windows.</p>
        </div>
        <button
          className="icon-button icon-button--close"
          aria-label="Close create context"
          disabled={saving}
          onClick={onClose}
        >
          ×
        </button>
      </header>
      <form className="sheet-form" onSubmit={(event) => void submit(event)}>
        {error && <SheetError error={error} />}
        <label className="field-label" htmlFor="context-name">Context name</label>
        <input
          id="context-name"
          className="text-field"
          value={name}
          autoFocus
          autoComplete="off"
          placeholder="e.g. coding"
          disabled={saving}
          onChange={(event) => setName(event.target.value)}
        />
        <p className="field-help">
          The name must be unique. You can add windows next, then maintain windows and URLs
          with Edit.
        </p>
        <div className="sheet-actions">
          <button type="button" className="button" disabled={saving} onClick={onClose}>
            Cancel
          </button>
          <button
            type="submit"
            className="button button--primary"
            disabled={!normalizedName || saving}
          >
            {saving ? "Creating…" : "Create context"}
          </button>
        </div>
      </form>
    </section>
  );
}

export function DeleteContextSheet({
  workspaces,
  activeWorkspace,
  onClose,
  onDeleted,
}: {
  workspaces: WorkspaceOverview[];
  activeWorkspace: string | null;
  onClose: () => void;
  onDeleted: (report: DeleteWorkspacesReport) => void;
}) {
  const defaultWorkspace = workspaces.some((workspace) => workspace.name === activeWorkspace)
    ? activeWorkspace ?? ""
    : workspaces[0]?.name ?? "";
  const [selected, setSelected] = useState(defaultWorkspace);
  const [saving, setSaving] = useState(false);
  const [confirmAll, setConfirmAll] = useState(false);
  const [error, setError] = useState<CommandError | null>(null);

  const removeOne = async () => {
    if (!selected) return;
    setSaving(true);
    setError(null);
    try {
      onDeleted(await deleteWorkspace(selected));
    } catch (cause) {
      setError(normalizeCommandError(cause));
    } finally {
      setSaving(false);
    }
  };

  const removeAll = async () => {
    if (!confirmAll) {
      setConfirmAll(true);
      return;
    }
    setSaving(true);
    setError(null);
    try {
      onDeleted(await deleteAllWorkspaces());
    } catch (cause) {
      setError(normalizeCommandError(cause));
      setConfirmAll(false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <section
      className="sheet sheet--compact"
      role="dialog"
      aria-modal="true"
      aria-labelledby="delete-context-title"
      onKeyDown={trapDialogFocus}
    >
      <header className="sheet-header">
        <div>
          <h2 id="delete-context-title">Delete contexts</h2>
          <p>Remove one context or clear the entire configuration.</p>
        </div>
        <button
          className="icon-button icon-button--close"
          aria-label="Close delete contexts"
          disabled={saving}
          onClick={onClose}
        >
          ×
        </button>
      </header>
      <div className="sheet-form">
        {error && <SheetError error={error} />}
        <label className="field-label" htmlFor="delete-context-name">Context</label>
        <select
          id="delete-context-name"
          className="text-field"
          value={selected}
          autoFocus
          disabled={saving || workspaces.length === 0}
          onChange={(event) => setSelected(event.target.value)}
        >
          {workspaces.map((workspace) => (
            <option key={workspace.name} value={workspace.name}>{workspace.name}</option>
          ))}
        </select>
        <p className="field-help">
          Deleting a context removes its saved windows, URLs, and runtime markers. It does
          not close any applications.
        </p>
        <button
          className="button button--danger button--full"
          disabled={!selected || saving}
          onClick={() => void removeOne()}
        >
          {saving ? "Deleting…" : `Delete “${selected || "context"}”`}
        </button>

        <div className="danger-zone">
          <strong>Delete all contexts</strong>
          <p>This keeps the Ctx configuration file but removes every context definition.</p>
          {confirmAll && (
            <p className="danger-confirmation">
              Click again to permanently delete all {workspaces.length} contexts.
            </p>
          )}
          <button
            className="button button--danger button--full"
            disabled={workspaces.length === 0 || saving}
            onClick={() => void removeAll()}
          >
            {saving
              ? "Deleting…"
              : confirmAll
                ? "Confirm delete all contexts"
                : "Delete all contexts"}
          </button>
        </div>
        <div className="sheet-actions">
          <button className="button" disabled={saving} onClick={onClose}>Cancel</button>
        </div>
      </div>
    </section>
  );
}
