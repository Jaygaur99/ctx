import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import type { CtxOverview } from "./types";

const api = vi.hoisted(() => ({
  addWindowsToWorkspace: vi.fn(),
  createWorkspace: vi.fn(),
  deleteAllWorkspaces: vi.fn(),
  deleteWorkspace: vi.fn(),
  editWorkspace: vi.fn(),
  getOverview: vi.fn(),
  getWindowCandidates: vi.fn(),
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
    api.getWindowCandidates.mockResolvedValue({ workspace: "coding", windows: [] });
    api.createWorkspace.mockResolvedValue({ workspace: "new-context", config_path: "/tmp/workspaces.yaml" });
    api.deleteWorkspace.mockResolvedValue({ deleted: ["coding"], active_workspace: null });
    api.deleteAllWorkspaces.mockResolvedValue({ deleted: ["coding", "research"], active_workspace: null });
    api.editWorkspace.mockResolvedValue({
      previous_name: "coding",
      workspace: "deep-work",
      urls: ["https://docs.rs/"],
      removed_windows: [42],
      already_absent_windows: [],
      active_workspace: "deep-work",
    });
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

  it("opens the in-popover window picker for a workspace", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "coding" });

    fireEvent.click(screen.getAllByRole("button", { name: /Add windows/ })[0]);

    expect(await screen.findByRole("dialog", { name: "Add windows" })).toBeInTheDocument();
    await waitFor(() => expect(api.getWindowCandidates).toHaveBeenCalledWith("coding"));
  });

  it("creates a context from the top control and proceeds to window selection", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "coding" });

    fireEvent.click(screen.getByRole("button", { name: "Create context" }));
    const dialog = screen.getByRole("dialog", { name: "Create context" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Context name" }), { target: { value: "new-context" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Create context" }));

    await waitFor(() => expect(api.createWorkspace).toHaveBeenCalledWith("new-context"));
    expect(await screen.findByRole("dialog", { name: "Add windows" })).toBeInTheDocument();
  });

  it("deletes one context from the top delete control", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "coding" });

    fireEvent.click(screen.getByRole("button", { name: "Delete contexts" }));
    fireEvent.click(screen.getByRole("button", { name: /Delete “coding”/ }));

    await waitFor(() => expect(api.deleteWorkspace).toHaveBeenCalledWith("coding"));
  });

  it("requires a second confirmation before deleting all contexts", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "coding" });

    fireEvent.click(screen.getByRole("button", { name: "Delete contexts" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete all contexts" }));
    expect(api.deleteAllWorkspaces).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Confirm delete all contexts" }));

    await waitFor(() => expect(api.deleteAllWorkspaces).toHaveBeenCalled());
  });

  it("edits a context definition through the shared core command", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "coding" });

    fireEvent.click(screen.getAllByRole("button", { name: "Edit" })[0]);
    const dialog = screen.getByRole("dialog", { name: "Edit context" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Context name" }), {
      target: { value: "deep-work" },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "＋ Add URL" }));
    fireEvent.change(within(dialog).getByRole("textbox", { name: "URL 1" }), {
      target: { value: "https://docs.rs" },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Remove" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Save context" }));
    expect(api.editWorkspace).not.toHaveBeenCalled();
    expect(within(dialog).getByText("Remove 1 tracked window?")).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Confirm removal & save" }));

    await waitFor(() => expect(api.editWorkspace).toHaveBeenCalledWith(
      "coding",
      "deep-work",
      ["https://docs.rs"],
      [42],
    ));
    await waitFor(() => expect(api.getOverview.mock.calls.length).toBeGreaterThanOrEqual(2));
    expect(screen.queryByRole("dialog", { name: "Edit context" })).not.toBeInTheDocument();
  });

  it("traps focus, restores it on close, and confirms before discarding dirty changes", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "coding" });

    const editTrigger = screen.getAllByRole("button", { name: "Edit" })[0];
    fireEvent.click(editTrigger);
    const dialog = screen.getByRole("dialog", { name: "Edit context" });
    const name = within(dialog).getByRole("textbox", { name: "Context name" });
    expect(name).toHaveFocus();
    expect(document.querySelector(".app-base")).toHaveAttribute("inert");
    expect(document.querySelector(".app-base")).toHaveAttribute("aria-hidden", "true");
    fireEvent.change(name, { target: { value: "deep-work" } });

    const close = within(dialog).getByRole("button", { name: "Close context editor" });
    const save = within(dialog).getByRole("button", { name: "Save context" });
    close.focus();
    fireEvent.keyDown(close, { key: "Tab", shiftKey: true });
    expect(save).toHaveFocus();
    fireEvent.keyDown(save, { key: "Tab" });
    expect(close).toHaveFocus();

    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(within(dialog).getByText("Discard unsaved changes?")).toBeInTheDocument();
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Edit context" })).not.toBeInTheDocument();
    expect(editTrigger).toHaveFocus();
  });

  it("removes and reorders URLs before saving", async () => {
    api.getOverview.mockResolvedValue({
      ...overview,
      workspaces: overview.workspaces.map((workspace) => workspace.name === "coding"
        ? {
            ...workspace,
            urls: ["https://one.test/", "https://two.test/"],
            url_statuses: [
              { url: "https://one.test/", state: "pending" as const },
              { url: "https://two.test/", state: "pending" as const },
            ],
            windows: [],
          }
        : workspace),
    });
    render(<App />);
    await screen.findByRole("heading", { name: "coding" });

    fireEvent.click(screen.getAllByRole("button", { name: "Edit" })[0]);
    const dialog = screen.getByRole("dialog", { name: "Edit context" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Move URL 1 down" }));
    expect(within(dialog).getByRole("textbox", { name: "URL 1" })).toHaveValue("https://two.test/");
    fireEvent.click(within(dialog).getByRole("button", { name: "Remove URL 1" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Save context" }));

    await waitFor(() => expect(api.editWorkspace).toHaveBeenCalledWith(
      "coding",
      "coding",
      ["https://one.test/"],
      [],
    ));
  });

  it("disables editor controls while a save is in flight", async () => {
    let finishSave: (() => void) | undefined;
    api.editWorkspace.mockImplementation(() => new Promise((resolve) => {
      finishSave = () => resolve({
        previous_name: "coding",
        workspace: "deep-work",
        urls: [],
        removed_windows: [],
        already_absent_windows: [],
        active_workspace: "deep-work",
      });
    }));
    render(<App />);
    await screen.findByRole("heading", { name: "coding" });

    fireEvent.click(screen.getAllByRole("button", { name: "Edit" })[0]);
    const dialog = screen.getByRole("dialog", { name: "Edit context" });
    const name = within(dialog).getByRole("textbox", { name: "Context name" });
    fireEvent.change(name, { target: { value: "deep-work" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save context" }));

    expect(name).toBeDisabled();
    expect(within(dialog).getByRole("button", { name: "Close context editor" })).toBeDisabled();
    expect(within(dialog).getByRole("button", { name: "＋ Add URL" })).toBeDisabled();
    expect(within(dialog).getByRole("button", { name: "Saving…" })).toBeDisabled();

    finishSave?.();
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Edit context" })).not.toBeInTheDocument());
  });

  it("labels stale tracked windows and confirms their removal", async () => {
    api.getOverview.mockResolvedValue({
      ...overview,
      workspaces: overview.workspaces.map((workspace) => workspace.name === "coding"
        ? {
            ...workspace,
            windows: workspace.windows.map((window) => ({
              ...window,
              resolved_id: null,
              pid: null,
              state: "missing" as const,
            })),
          }
        : workspace),
    });
    render(<App />);
    await screen.findByRole("heading", { name: "coding" });

    fireEvent.click(screen.getAllByRole("button", { name: "Edit" })[0]);
    const dialog = screen.getByRole("dialog", { name: "Edit context" });
    expect(within(dialog).getByText("Code · missing")).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "Remove" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Save context" }));
    expect(api.editWorkspace).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Confirm removal & save" }));

    await waitFor(() => expect(api.editWorkspace).toHaveBeenCalledWith(
      "coding",
      "coding",
      [],
      [42],
    ));
  });

  it("keeps core validation failures inside the editor", async () => {
    api.editWorkspace.mockRejectedValue({
      code: "config",
      message: "workspace 'research' already exists",
    });
    render(<App />);
    await screen.findByRole("heading", { name: "coding" });

    fireEvent.click(screen.getAllByRole("button", { name: "Edit" })[0]);
    const dialog = screen.getByRole("dialog", { name: "Edit context" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Context name" }), {
      target: { value: "research" },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save context" }));

    expect(await within(dialog).findByText("workspace 'research' already exists")).toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "Edit context" })).toBeInTheDocument();
  });

  it("renders clear empty states for a context without URLs or windows", async () => {
    api.getOverview.mockResolvedValue({
      ...overview,
      active_workspace: "empty",
      workspaces: [{
        name: "empty",
        active: true,
        path: null,
        services: [],
        urls: [],
        url_statuses: [],
        windows: [],
      }],
    });
    render(<App />);
    await screen.findByRole("heading", { name: "empty" });

    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    const dialog = screen.getByRole("dialog", { name: "Edit context" });
    expect(within(dialog).getByText("No URLs configured.")).toBeInTheDocument();
    expect(within(dialog).getByText("No windows tracked.")).toBeInTheDocument();
  });
});
