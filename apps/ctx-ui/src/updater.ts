import { check, type DownloadEvent } from "@tauri-apps/plugin-updater";
import { restartCtx } from "./api";

export interface UpdateProgress {
  downloadedBytes: number;
  totalBytes: number | null;
  percent: number | null;
}

export interface AvailableUpdate {
  currentVersion: string;
  version: string;
  date?: string;
  body?: string;
  install: (onProgress: (progress: UpdateProgress) => void) => Promise<void>;
}

function progressSnapshot(
  event: DownloadEvent,
  downloadedBytes: number,
  totalBytes: number | null,
): UpdateProgress {
  if (event.event === "Started") {
    totalBytes = event.data.contentLength ?? null;
  } else if (event.event === "Progress") {
    downloadedBytes += event.data.chunkLength;
  }

  return {
    downloadedBytes,
    totalBytes,
    percent:
      totalBytes && totalBytes > 0
        ? Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))
        : null,
  };
}

export async function checkForUpdate(): Promise<AvailableUpdate | null> {
  const update = await check();
  if (!update) return null;

  return {
    currentVersion: update.currentVersion,
    version: update.version,
    date: update.date,
    body: update.body,
    install: async (onProgress) => {
      let downloadedBytes = 0;
      let totalBytes: number | null = null;

      await update.downloadAndInstall((event) => {
        const progress = progressSnapshot(event, downloadedBytes, totalBytes);
        downloadedBytes = progress.downloadedBytes;
        totalBytes = progress.totalBytes;
        onProgress(progress);
      });
      await restartCtx();
    },
  };
}
