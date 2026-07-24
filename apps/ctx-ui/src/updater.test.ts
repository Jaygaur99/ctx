import { beforeEach, describe, expect, it, vi } from "vitest";
import { checkForUpdate } from "./updater";

const plugin = vi.hoisted(() => ({
  check: vi.fn(),
}));

const api = vi.hoisted(() => ({
  restartCtx: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-updater", () => plugin);
vi.mock("./api", () => api);

describe("Ctx updater", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.restartCtx.mockResolvedValue(undefined);
  });

  it("reports when no newer release exists", async () => {
    plugin.check.mockResolvedValue(null);

    await expect(checkForUpdate()).resolves.toBeNull();
  });

  it("downloads, reports progress, installs, and restarts", async () => {
    const downloadAndInstall = vi.fn(async (onEvent: (event: unknown) => void) => {
      onEvent({ event: "Started", data: { contentLength: 100 } });
      onEvent({ event: "Progress", data: { chunkLength: 40 } });
      onEvent({ event: "Progress", data: { chunkLength: 60 } });
      onEvent({ event: "Finished" });
    });
    plugin.check.mockResolvedValue({
      currentVersion: "1.0.0",
      version: "1.0.1",
      date: "2026-07-25",
      body: "Small fixes",
      downloadAndInstall,
    });
    const progress = vi.fn();

    const update = await checkForUpdate();
    await update?.install(progress);

    expect(update).toMatchObject({
      currentVersion: "1.0.0",
      version: "1.0.1",
      body: "Small fixes",
    });
    expect(progress).toHaveBeenLastCalledWith({
      downloadedBytes: 100,
      totalBytes: 100,
      percent: 100,
    });
    expect(api.restartCtx).toHaveBeenCalledTimes(1);
  });

  it("does not restart after an installation failure", async () => {
    plugin.check.mockResolvedValue({
      currentVersion: "1.0.0",
      version: "1.0.1",
      downloadAndInstall: vi.fn().mockRejectedValue(new Error("invalid signature")),
    });

    const update = await checkForUpdate();

    await expect(update?.install(vi.fn())).rejects.toThrow("invalid signature");
    expect(api.restartCtx).not.toHaveBeenCalled();
  });
});
