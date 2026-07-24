import { useCallback, useEffect, useState } from "react";
import { normalizeCommandError } from "../api";
import {
  checkForUpdate,
  type AvailableUpdate,
  type UpdateProgress,
} from "../updater";

export type UpdateStatus =
  | "checking"
  | "current"
  | "available"
  | "installing"
  | "installed"
  | "error";

export interface UpdateManager {
  status: UpdateStatus;
  availableUpdate: AvailableUpdate | null;
  progress: UpdateProgress | null;
  error: string | null;
  refresh: () => Promise<void>;
  install: () => Promise<void>;
}

export function useUpdateManager(): UpdateManager {
  const [status, setStatus] = useState<UpdateStatus>("checking");
  const [availableUpdate, setAvailableUpdate] = useState<AvailableUpdate | null>(null);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setStatus("checking");
    setAvailableUpdate(null);
    setProgress(null);
    setError(null);
    try {
      const update = await checkForUpdate();
      setAvailableUpdate(update);
      setStatus(update ? "available" : "current");
    } catch (cause) {
      setError(normalizeCommandError(cause).message);
      setStatus("error");
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const install = useCallback(async () => {
    if (!availableUpdate) return;
    setStatus("installing");
    setProgress(null);
    setError(null);
    try {
      await availableUpdate.install(setProgress);
      setStatus("installed");
    } catch (cause) {
      setError(normalizeCommandError(cause).message);
      setStatus("available");
    }
  }, [availableUpdate]);

  return {
    status,
    availableUpdate,
    progress,
    error,
    refresh,
    install,
  };
}
