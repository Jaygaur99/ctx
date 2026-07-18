import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AddWindowsReport,
  CommandError,
  CtxOverview,
  SwitchReport,
  UrlLaunchReport,
  WindowPickerOverview,
} from "./types";

export const getOverview = () => invoke<CtxOverview>("get_overview");
export const switchWorkspace = (name: string) =>
  invoke<SwitchReport>("switch_workspace", { name });
export const openWorkspaceUrls = (name: string) =>
  invoke<UrlLaunchReport>("open_workspace_urls", { name });
export const hidePopover = () => invoke<void>("hide_popover");
export const showPopover = () => invoke<void>("show_popover");
export const showWindowPicker = (workspace: string) =>
  invoke<void>("show_window_picker", { workspace });
export const getWindowPickerWorkspace = () =>
  invoke<string>("get_window_picker_workspace");
export const hideWindowPicker = () => invoke<void>("hide_window_picker");
export const getWindowCandidates = (workspace: string) =>
  invoke<WindowPickerOverview>("get_window_candidates", { workspace });
export const addWindowsToWorkspace = (workspace: string, windowIds: number[]) =>
  invoke<AddWindowsReport>("add_windows_to_workspace", { workspace, windowIds });
export const quitCtx = () => invoke<void>("quit");
export const onPopoverOpened = (handler: () => void): Promise<UnlistenFn> =>
  listen("ctx://popover-opened", handler);
export const onWindowPickerOpened = (handler: (workspace: string) => void): Promise<UnlistenFn> =>
  listen<string>("ctx://window-picker-opened", (event) => handler(event.payload));

export function normalizeCommandError(error: unknown): CommandError {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    "message" in error &&
    typeof error.code === "string" &&
    typeof error.message === "string"
  ) {
    return { code: error.code, message: error.message };
  }
  return {
    code: "unknown",
    message: error instanceof Error ? error.message : String(error),
  };
}
