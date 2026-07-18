import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import WindowPicker from "./WindowPicker";
import type { WindowPickerOverview } from "./types";

const api = vi.hoisted(() => ({
  addWindowsToWorkspace: vi.fn(),
  getWindowCandidates: vi.fn(),
  getWindowPickerWorkspace: vi.fn(),
  hideWindowPicker: vi.fn(),
  onWindowPickerOpened: vi.fn(),
  showPopover: vi.fn(),
}));

vi.mock("./api", () => ({
  ...api,
  normalizeCommandError: (error: unknown) => error,
}));

const picker: WindowPickerOverview = {
  workspace: "coding",
  windows: [
    {
      id: 42,
      pid: 7,
      application: "Code",
      title: "Ctx editor",
      bounds: { x: 10, y: 20, width: 900, height: 700 },
      assigned_to: [],
      already_in_workspace: false,
    },
    {
      id: 57,
      pid: 8,
      application: "Terminal",
      title: "server",
      bounds: null,
      assigned_to: ["coding"],
      already_in_workspace: true,
    },
  ],
};

describe("Ctx window picker", () => {
  afterEach(cleanup);

  beforeEach(() => {
    vi.clearAllMocks();
    api.getWindowPickerWorkspace.mockResolvedValue("coding");
    api.getWindowCandidates.mockResolvedValue(picker);
    api.onWindowPickerOpened.mockResolvedValue(() => undefined);
    api.addWindowsToWorkspace.mockResolvedValue({
      workspace: "coding",
      added: [{ id: 42, pid: 7, owner: "Code", title: "Ctx editor", placement: null, placement_warning: null }],
      already_tracked: [],
    });
    api.hideWindowPicker.mockResolvedValue(undefined);
    api.showPopover.mockResolvedValue(undefined);
  });

  it("lists live windows and disables windows already in the workspace", async () => {
    render(<WindowPicker />);

    expect(await screen.findByText("Ctx editor")).toBeInTheDocument();
    expect(screen.getByText("Already in this workspace")).toBeInTheDocument();
    const checkboxes = screen.getAllByRole("checkbox");
    expect(checkboxes[0]).toBeEnabled();
    expect(checkboxes[1]).toBeDisabled();
  });

  it("adds the selected windows and returns to the popover", async () => {
    render(<WindowPicker />);
    await screen.findByText("Ctx editor");

    fireEvent.click(screen.getAllByRole("checkbox")[0]);
    fireEvent.click(screen.getByRole("button", { name: "Add 1 window" }));

    await waitFor(() => expect(api.addWindowsToWorkspace).toHaveBeenCalledWith("coding", [42]));
    expect(api.hideWindowPicker).toHaveBeenCalled();
    expect(api.showPopover).toHaveBeenCalled();
  });

  it("filters candidates by application and title", async () => {
    render(<WindowPicker />);
    await screen.findByText("Ctx editor");

    fireEvent.change(screen.getByRole("searchbox", { name: "Filter windows" }), {
      target: { value: "terminal" },
    });

    expect(screen.queryByText("Ctx editor")).not.toBeInTheDocument();
    expect(screen.getByText("server")).toBeInTheDocument();
  });
});
