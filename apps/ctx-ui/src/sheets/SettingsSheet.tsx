import { useCallback, useEffect, useState } from "react";
import {
  getAppSettings,
  normalizeCommandError,
  openSettingsTarget,
  setLaunchAtLogin,
} from "../api";
import type {
  AppSettings,
  CommandError,
  SettingsTarget,
  ThemePreference,
} from "../types";
import { trapDialogFocus } from "../dialogFocus";
import UpdateSettingsSection from "../components/UpdateSettingsSection";
import { useUpdateManager } from "../hooks/useUpdateManager";

function PermissionRow({
  label,
  description,
  granted,
  target,
  busyTarget,
  onOpen,
}: {
  label: string;
  description: string;
  granted: boolean;
  target: SettingsTarget;
  busyTarget: SettingsTarget | null;
  onOpen: (target: SettingsTarget) => void;
}) {
  return (
    <div className="settings-card settings-card--permission">
      <div className="settings-card__heading">
        <div>
          <strong>{label}</strong>
          <span>{description}</span>
        </div>
        <span className={`state-tag state-tag--${granted ? "good" : "warning"}`}>
          {granted ? "Allowed" : "Needs access"}
        </span>
      </div>
      <button
        type="button"
        className="button"
        disabled={busyTarget !== null}
        onClick={() => onOpen(target)}
      >
        {busyTarget === target ? "Opening…" : "Open System Settings"}
      </button>
    </div>
  );
}

export default function SettingsSheet({
  onClose,
  returnFocus,
  simpleMode,
  theme,
  onSimpleModeChange,
  onThemeChange,
}: {
  onClose: () => void;
  returnFocus: HTMLButtonElement | null;
  simpleMode: boolean;
  theme: ThemePreference;
  onSimpleModeChange: (enabled: boolean) => void;
  onThemeChange: (theme: ThemePreference) => void;
}) {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [busyTarget, setBusyTarget] = useState<SettingsTarget | null>(null);
  const [error, setError] = useState<CommandError | null>(null);
  const update = useUpdateManager();
  const installingUpdate = update.status === "installing";

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setSettings(await getAppSettings());
    } catch (cause) {
      setError(normalizeCommandError(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => () => {
    if (returnFocus?.isConnected) returnFocus.focus();
  }, [returnFocus]);

  const close = () => {
    if (!saving && busyTarget === null && !installingUpdate) onClose();
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      close();
      return;
    }
    trapDialogFocus(event);
  };

  const updateLaunchAtLogin = async (enabled: boolean) => {
    setSaving(true);
    setError(null);
    try {
      setSettings(await setLaunchAtLogin(enabled));
    } catch (cause) {
      setError(normalizeCommandError(cause));
    } finally {
      setSaving(false);
    }
  };

  const openTarget = async (target: SettingsTarget) => {
    setBusyTarget(target);
    setError(null);
    try {
      await openSettingsTarget(target);
    } catch (cause) {
      setError(normalizeCommandError(cause));
    } finally {
      setBusyTarget(null);
    }
  };

  return (
    <section
      className="sheet sheet--compact"
      role="dialog"
      aria-modal="true"
      aria-labelledby="settings-title"
      onKeyDown={handleKeyDown}
    >
      <header className="sheet-header">
        <div>
          <h2 id="settings-title">Settings</h2>
          <p>Appearance, startup, updates, and app information.</p>
        </div>
        <button
          className="icon-button icon-button--close"
          aria-label="Close settings"
          disabled={saving || busyTarget !== null || installingUpdate}
          autoFocus
          onClick={close}
        >
          ×
        </button>
      </header>

      <div className="sheet-form settings-content" aria-live="polite">
        {error && (
          <div className="banner banner--danger" role="alert">
            <strong>Couldn’t update settings</strong>
            <p>{error.message}</p>
            {!settings && <button className="text-button" onClick={() => void refresh()}>Try again</button>}
          </div>
        )}
        {loading && !settings && !error && <div className="settings-loading">Loading settings…</div>}

        {settings && (
          <>
            <section className="settings-section" aria-labelledby="appearance-settings-title">
              <div className="settings-section__heading">
                <h3 id="appearance-settings-title">Appearance</h3>
              </div>
              <div className="settings-list">
                <label className="settings-toggle">
                  <span>
                    <strong>Simple view</strong>
                    <small>Hide window, recovery, placement, URL, and diagnostic details.</small>
                  </span>
                  <input
                    type="checkbox"
                    role="switch"
                    aria-label="Simple view"
                    checked={simpleMode}
                    onChange={(event) => onSimpleModeChange(event.target.checked)}
                  />
                </label>
                <label className="settings-card settings-card--select">
                  <div>
                    <strong>Theme</strong>
                    <small>System follows your current macOS appearance.</small>
                  </div>
                  <select
                    className="settings-select"
                    aria-label="Theme"
                    value={theme}
                    onChange={(event) => onThemeChange(event.target.value as ThemePreference)}
                  >
                    <option value="system">System</option>
                    <option value="light">Light</option>
                    <option value="dark">Dark</option>
                  </select>
                </label>
              </div>
            </section>

            <section className="settings-section" aria-labelledby="startup-settings-title">
              <div className="settings-section__heading">
                <h3 id="startup-settings-title">Startup</h3>
              </div>
              <label className="settings-toggle">
                <span>
                  <strong>Launch at login</strong>
                  <small>Start Ctx quietly in the menu bar when you sign in.</small>
                </span>
                <input
                  type="checkbox"
                  role="switch"
                  aria-label="Launch at login"
                  checked={settings.launch_at_login}
                  disabled={saving || busyTarget !== null}
                  onChange={(event) => void updateLaunchAtLogin(event.target.checked)}
                />
              </label>
            </section>

            <section className="settings-section" aria-labelledby="permission-settings-title">
              <div className="settings-section__heading">
                <h3 id="permission-settings-title">Permissions</h3>
                <button className="text-button" disabled={loading || saving || busyTarget !== null} onClick={() => void refresh()}>
                  {loading ? "Refreshing…" : "Refresh status"}
                </button>
              </div>
              <div className="settings-list">
                <PermissionRow
                  label="Screen Recording"
                  description="Lets Ctx identify windows across apps and Desktops."
                  granted={settings.permissions.screen_recording}
                  target="screen_recording"
                  busyTarget={busyTarget}
                  onOpen={(target) => void openTarget(target)}
                />
                <PermissionRow
                  label="Accessibility"
                  description="Lets Ctx minimize, restore, and place windows."
                  granted={settings.permissions.accessibility}
                  target="accessibility"
                  busyTarget={busyTarget}
                  onOpen={(target) => void openTarget(target)}
                />
              </div>
            </section>

            <section className="settings-section" aria-labelledby="files-settings-title">
              <div className="settings-section__heading">
                <h3 id="files-settings-title">Files</h3>
              </div>
              <div className="settings-card">
                <div>
                  <strong>Configuration folder</strong>
                  <code title={settings.config_folder}>{settings.config_folder}</code>
                </div>
                <button
                  className="button"
                  disabled={busyTarget !== null}
                  onClick={() => void openTarget("config_folder")}
                >
                  {busyTarget === "config_folder" ? "Opening…" : "Open Config Folder"}
                </button>
              </div>
            </section>

            <UpdateSettingsSection
              currentVersion={settings.version}
              update={update}
            />

            <section className="settings-section" aria-labelledby="about-settings-title">
              <div className="settings-section__heading">
                <h3 id="about-settings-title">About</h3>
              </div>
              <div className="settings-card">
                <div>
                  <strong>Ctx {settings.version}</strong>
                  <span>{settings.build} build</span>
                </div>
                <button
                  className="button"
                  title={settings.release_url}
                  disabled={busyTarget !== null}
                  onClick={() => void openTarget("latest_release")}
                >
                  {busyTarget === "latest_release" ? "Opening…" : "View Latest Release"}
                </button>
              </div>
            </section>
          </>
        )}
      </div>
    </section>
  );
}
