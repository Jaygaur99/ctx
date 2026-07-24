import type { UpdateManager } from "../hooks/useUpdateManager";

export default function UpdateSettingsSection({
  currentVersion,
  update,
}: {
  currentVersion: string;
  update: UpdateManager;
}) {
  const {
    status,
    availableUpdate,
    progress,
    error,
    refresh,
    install,
  } = update;

  return (
    <section className="settings-section" aria-labelledby="update-settings-title">
      <div className="settings-section__heading">
        <h3 id="update-settings-title">Updates</h3>
      </div>
      <div className="settings-card settings-card--update">
        <div>
          <strong>
            {status === "checking" && "Checking for updates…"}
            {status === "current" && "Ctx is up to date"}
            {status === "available" && `Ctx ${availableUpdate?.version ?? ""} is available`}
            {status === "installing" && "Installing update…"}
            {status === "installed" && "Update installed"}
            {status === "error" && "Couldn’t check for updates"}
          </strong>
          <span>
            {status === "current" && `You’re running Ctx ${currentVersion}.`}
            {status === "available"
              && (error ?? availableUpdate?.body ?? "Ready to download and install.")}
            {status === "installing"
              && (progress?.percent != null
                ? `${progress.percent}% downloaded`
                : "Downloading and verifying the signed update.")}
            {status === "installed" && "Ctx is restarting to finish the update."}
            {status === "error" && error}
            {status === "checking" && "Looking at the latest GitHub Release."}
          </span>
        </div>
        {status === "available" && (
          <button
            className="button button--primary"
            onClick={() => void install()}
          >
            {error ? "Retry Install" : "Install Update"}
          </button>
        )}
        {(status === "current" || status === "error") && (
          <button className="button" onClick={() => void refresh()}>
            Check Again
          </button>
        )}
        {status === "installing" && (
          <button className="button" disabled>
            Installing…
          </button>
        )}
      </div>
    </section>
  );
}
