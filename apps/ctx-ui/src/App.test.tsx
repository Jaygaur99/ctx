import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import type { CtxOverview } from "./types";

const api = vi.hoisted(() => ({
  getOverview: vi.fn(),
  hidePopover: vi.fn(),
  onPopoverOpened: vi.fn(),
  openWorkspaceUrls: vi.fn(),
  quitCtx: vi.fn(),
  showPopover: vi.fn(),
  switchWorkspace: vi.fn(),
}));

vi.mock("./api", () => ({
  ...api,
  normalizeCommandError: (error: unknown) => error,
}));

const overview: CtxOverview = {
  config_path: "/tmp/workspaces.yaml",
  active_workspace: "coding",
  workspaces: [
    {
      name: "research",
      active: false,
      path: "/tmp/research",
      services: [],
      urls: ["https://example.com/"],
      url_statuses: [{ url: "https://example.com/", state: "pending" }],
      windows: [],
    },
    {
      name: "coding",
      active: true,
      path: "/tmp/coding",
      services: [],
      urls: [],
      url_statuses: [],
      windows: [
        {
          saved_id: 42,
          resolved_id: 42,
          pid: 7,
          owner: "Code",
          title: "Ctx",
          state: "visible",
          recovery_kind: "editor",
          recovery_ready: true,
          recovery_degraded: false,
          recovery_warning: null,
          placement: { display_uuid: "main", desktop_ordinal: 2 },
          placement_degraded: false,
          placement_warning: null,
        },
      ],
    },
  ],
};

describe("Ctx popover", () => {
  afterEach(cleanup);

  beforeEach(() => {
    vi.clearAllMocks();
    api.getOverview.mockResolvedValue(overview);
    api.hidePopover.mockResolvedValue(undefined);
    api.showPopover.mockResolvedValue(undefined);
    api.onPopoverOpened.mockResolvedValue(() => undefined);
    api.switchWorkspace.mockResolvedValue({
      urls: { workspace: "research", opened: [], already_opened: [], recovery_managed: [], failed: [] },
    });
    api.openWorkspaceUrls.mockResolvedValue({
      workspace: "research", opened: ["https://example.com/"], already_opened: [], recovery_managed: [], failed: [],
    });
  });

  it("renders the active workspace first with detailed state", async () => {
    render(<App />);

    expect(await screen.findByRole("heading", { name: "coding" })).toBeInTheDocument();
    const headings = screen.getAllByRole("heading", { level: 2 });
    expect(headings.map((heading) => heading.textContent)).toEqual(["coding", "research"]);
    fireEvent.click(screen.getAllByText("Details")[0]);
    expect(screen.getByText("Ctx", { selector: ".detail-item strong" })).toBeInTheDocument();
    expect(screen.getByText("Desktop 2")).toBeInTheDocument();
  });

  it("hides and switches through the typed API", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "research" });

    fireEvent.click(screen.getByRole("button", { name: "Switch" }));

    await waitFor(() => expect(api.switchWorkspace).toHaveBeenCalledWith("research"));
    expect(api.hidePopover).toHaveBeenCalled();
  });

  it("reopens the popover for partial URL failures", async () => {
    api.openWorkspaceUrls.mockResolvedValue({
      workspace: "research",
      opened: [],
      already_opened: [],
      recovery_managed: [],
      failed: [{ url: "https://example.com/", error: "offline" }],
    });
    render(<App />);
    await screen.findByRole("heading", { name: "research" });

    fireEvent.click(screen.getByRole("button", { name: "Open URLs" }));

    await waitFor(() => expect(api.showPopover).toHaveBeenCalled());
    expect(await screen.findByText(/could not be opened/)).toBeInTheDocument();
  });
});
