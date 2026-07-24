import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import SettingsSheet from "./SettingsSheet";
import type { AppSettings } from "./types";

const api = vi.hoisted(() => ({
  getAppSettings: vi.fn(),
  openSettingsTarget: vi.fn(),
  setLaunchAtLogin: vi.fn(),
}));

const updater = vi.hoisted(() => ({
  checkForUpdate: vi.fn(),
}));

vi.mock("./api", () => ({
  ...api,
  normalizeCommandError: (error: unknown) => error,
}));

vi.mock("./updater", () => updater);

const settings: AppSettings = {
  launch_at_login: false,
  permissions: {
    screen_recording: true,
    accessibility: false,
  },
  config_folder: "/Users/test/Library/Application Support/ctx",
  version: "0.3.0",
  build: "Development",
  release_url: "https://github.com/Jaygaur99/ctx/releases/latest",
};

describe("Ctx settings", () => {
  afterEach(cleanup);

  beforeEach(() => {
    vi.clearAllMocks();
    api.getAppSettings.mockResolvedValue(settings);
    api.openSettingsTarget.mockResolvedValue(undefined);
    api.setLaunchAtLogin.mockResolvedValue({
      ...settings,
      launch_at_login: true,
    });
    updater.checkForUpdate.mockResolvedValue(null);
  });

  it("reports startup, permission, config, and build state", async () => {
    render(<SettingsSheet onClose={vi.fn()} returnFocus={null} />);

    expect(await screen.findByRole("switch", { name: "Launch at login" })).not.toBeChecked();
    expect(screen.getByText("Screen Recording").closest(".settings-card")).toHaveTextContent("Allowed");
    expect(screen.getByText("Accessibility").closest(".settings-card")).toHaveTextContent("Needs access");
    expect(screen.getByText(settings.config_folder)).toBeInTheDocument();
    expect(screen.getByText("Ctx 0.3.0")).toBeInTheDocument();
    expect(screen.getByText("Development build")).toBeInTheDocument();
    expect(await screen.findByText("Ctx is up to date")).toBeInTheDocument();
  });

  it("persists and displays the verified launch-at-login state", async () => {
    render(<SettingsSheet onClose={vi.fn()} returnFocus={null} />);
    const toggle = await screen.findByRole("switch", { name: "Launch at login" });

    fireEvent.click(toggle);

    await waitFor(() => expect(api.setLaunchAtLogin).toHaveBeenCalledWith(true));
    await waitFor(() => expect(toggle).toBeChecked());
  });

  it("keeps the previous state and shows launch-at-login failures inline", async () => {
    api.setLaunchAtLogin.mockRejectedValue({
      code: "settings",
      message: "launch agent is unavailable",
    });
    render(<SettingsSheet onClose={vi.fn()} returnFocus={null} />);
    const toggle = await screen.findByRole("switch", { name: "Launch at login" });

    fireEvent.click(toggle);

    expect(await screen.findByText("launch agent is unavailable")).toBeInTheDocument();
    expect(toggle).not.toBeChecked();
  });

  it("opens only the typed settings destinations", async () => {
    render(<SettingsSheet onClose={vi.fn()} returnFocus={null} />);
    await screen.findByRole("switch", { name: "Launch at login" });

    fireEvent.click(screen.getAllByRole("button", { name: "Open System Settings" })[0]);
    await waitFor(() => expect(api.openSettingsTarget).toHaveBeenCalledWith("screen_recording"));
    fireEvent.click(screen.getAllByRole("button", { name: "Open System Settings" })[1]);
    await waitFor(() => expect(api.openSettingsTarget).toHaveBeenCalledWith("accessibility"));
    fireEvent.click(screen.getByRole("button", { name: "Open Config Folder" }));
    await waitFor(() => expect(api.openSettingsTarget).toHaveBeenCalledWith("config_folder"));
    fireEvent.click(screen.getByRole("button", { name: "View Latest Release" }));
    await waitFor(() => expect(api.openSettingsTarget).toHaveBeenCalledWith("latest_release"));
  });

  it("finds and installs a signed update from settings", async () => {
    const install = vi.fn(async (onProgress: (progress: {
      downloadedBytes: number;
      totalBytes: number | null;
      percent: number | null;
    }) => void) => {
      onProgress({ downloadedBytes: 50, totalBytes: 100, percent: 50 });
    });
    updater.checkForUpdate.mockResolvedValue({
      currentVersion: "1.0.0",
      version: "1.0.1",
      body: "Small fixes",
      install,
    });

    render(<SettingsSheet onClose={vi.fn()} returnFocus={null} />);

    expect(await screen.findByText("Ctx 1.0.1 is available")).toBeInTheDocument();
    expect(screen.getByText("Small fixes")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Install Update" }));

    await waitFor(() => expect(install).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("Update installed")).toBeInTheDocument();
  });

  it("keeps updater failures separate and retryable", async () => {
    updater.checkForUpdate.mockRejectedValue(new Error("release server unavailable"));

    render(<SettingsSheet onClose={vi.fn()} returnFocus={null} />);

    expect(await screen.findByText("Couldn’t check for updates")).toBeInTheDocument();
    expect(screen.getByText("release server unavailable")).toBeInTheDocument();

    updater.checkForUpdate.mockResolvedValue(null);
    fireEvent.click(screen.getByRole("button", { name: "Check Again" }));

    expect(await screen.findByText("Ctx is up to date")).toBeInTheDocument();
  });
});
