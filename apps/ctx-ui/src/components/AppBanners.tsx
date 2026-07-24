import type {
  CommandError,
  UrlLaunchFailure,
  WindowActionFailure,
} from "../types";

export function ErrorBanner({
  error,
  onRetry,
}: {
  error: CommandError;
  onRetry: () => void;
}) {
  return (
    <div
      className={`banner banner--${error.code === "permission" ? "warning" : "danger"}`}
      role="alert"
    >
      <strong>
        {error.code === "permission"
          ? "Permission required"
          : "Ctx couldn’t complete that action"}
      </strong>
      <p>{error.message}</p>
      <button className="text-button" onClick={onRetry}>Try again</button>
    </div>
  );
}

export function PartialFailureBanner({
  failures,
}: {
  failures: UrlLaunchFailure[];
}) {
  return (
    <div className="banner banner--warning" role="status">
      <strong>
        {failures.length} URL{failures.length === 1 ? "" : "s"} could not be opened
      </strong>
      {failures.map((failure) => (
        <p key={failure.url}>{failure.url}: {failure.error}</p>
      ))}
    </div>
  );
}

export function WindowFailureBanner({
  failures,
}: {
  failures: WindowActionFailure[];
}) {
  return (
    <div className="banner banner--warning" role="status">
      <strong>
        {failures.length} window{failures.length === 1 ? "" : "s"} could not be hidden
      </strong>
      {failures.map((failure) => (
        <p key={`${failure.owner}-${failure.id}`}>
          {failure.owner}: {failure.error}
        </p>
      ))}
    </div>
  );
}
