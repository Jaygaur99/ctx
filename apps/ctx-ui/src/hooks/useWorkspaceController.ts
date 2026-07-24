import { useCallback, useEffect, useMemo, useState } from "react";
import {
  getOverview,
  hideAllExceptActive,
  hidePopover,
  normalizeCommandError,
  onPopoverOpened,
  openWorkspaceUrls,
  showPopover,
  switchWorkspace,
} from "../api";
import type {
  CommandError,
  CtxOverview,
  UrlLaunchFailure,
  WindowActionFailure,
} from "../types";

type BusyAction =
  | { workspace: string; action: "switch" | "open" }
  | { action: "hide_all" }
  | null;

export function useWorkspaceController() {
  const [overview, setOverview] = useState<CtxOverview | null>(null);
  const [error, setError] = useState<CommandError | null>(null);
  const [partialFailures, setPartialFailures] = useState<UrlLaunchFailure[]>([]);
  const [windowFailures, setWindowFailures] = useState<WindowActionFailure[]>([]);
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

  const orderedWorkspaces = useMemo(
    () => [...(overview?.workspaces ?? [])].sort((first, second) => {
      if (first.active !== second.active) return first.active ? -1 : 1;
      return first.name.localeCompare(second.name);
    }),
    [overview],
  );

  const runWorkspaceAction = async (
    workspace: string,
    action: "switch" | "open",
  ) => {
    setBusy({ workspace, action });
    setError(null);
    setPartialFailures([]);
    setWindowFailures([]);
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

  const runHideAll = async () => {
    setBusy({ action: "hide_all" });
    setError(null);
    setPartialFailures([]);
    setWindowFailures([]);
    await hidePopover().catch(() => undefined);
    try {
      const report = await hideAllExceptActive();
      setBusy(null);
      await refresh();
      if (report.skipped.length > 0) {
        setWindowFailures(report.skipped);
        await showPopover();
      }
    } catch (cause) {
      setBusy(null);
      setError(normalizeCommandError(cause));
      await showPopover().catch(() => undefined);
    }
  };

  return {
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
  };
}
