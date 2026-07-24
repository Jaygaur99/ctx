export type WindowState = "visible" | "minimized" | "ambiguous" | "missing";
export type RecoveryKind = "editor" | "terminal" | "browser" | "generic";
export type WorkspaceUrlState = "pending" | "opened" | "recovery_managed" | "failed";

export interface DesktopPlacement {
  display_uuid: string;
  desktop_ordinal: number;
}

export interface WindowBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface WindowCandidate {
  id: number;
  pid: number;
  application: string;
  title: string | null;
  bounds: WindowBounds | null;
  assigned_to: string[];
  already_in_workspace: boolean;
}

export interface WindowPickerOverview {
  workspace: string;
  windows: WindowCandidate[];
}

export interface AddedWindow {
  id: number;
  pid: number;
  owner: string;
  title: string | null;
  placement: DesktopPlacement | null;
  placement_warning: string | null;
}

export interface AddWindowsReport {
  workspace: string;
  added: AddedWindow[];
  already_tracked: number[];
}

export interface CreateWorkspaceReport {
  workspace: string;
  config_path: string;
}

export interface DeleteWorkspacesReport {
  deleted: string[];
  active_workspace: string | null;
}

export interface EditWorkspaceReport {
  previous_name: string;
  workspace: string;
  urls: string[];
  removed_windows: number[];
  already_absent_windows: number[];
  active_workspace: string | null;
}

export interface WindowStatus {
  saved_id: number;
  resolved_id: number | null;
  pid: number | null;
  owner: string;
  title: string | null;
  state: WindowState;
  recovery_kind: RecoveryKind | null;
  recovery_ready: boolean;
  recovery_degraded: boolean;
  recovery_warning: string | null;
  placement: DesktopPlacement | null;
  placement_degraded: boolean;
  placement_warning: string | null;
}

export interface WorkspaceUrlStatus {
  url: string;
  state: WorkspaceUrlState;
  error?: string;
}

export interface WorkspaceOverview {
  name: string;
  active: boolean;
  path: string | null;
  services: unknown[];
  windows: WindowStatus[];
  urls: string[];
  url_statuses: WorkspaceUrlStatus[];
}

export interface CtxOverview {
  config_path: string;
  active_workspace: string | null;
  workspaces: WorkspaceOverview[];
}

export interface UrlLaunchFailure {
  url: string;
  error: string;
}

export interface UrlLaunchReport {
  workspace: string;
  opened: string[];
  already_opened: string[];
  recovery_managed: string[];
  failed: UrlLaunchFailure[];
}

export interface SwitchReport {
  urls: UrlLaunchReport;
}

export interface WindowActionFailure {
  id: number;
  owner: string;
  error: string;
}

export interface HideAllReport {
  active_workspace: string;
  protected: number[];
  hidden: number[];
  skipped: WindowActionFailure[];
}

export interface CommandError {
  code: string;
  message: string;
}

export interface AppSettings {
  launch_at_login: boolean;
  permissions: {
    screen_recording: boolean;
    accessibility: boolean;
  };
  config_folder: string;
  version: string;
  build: string;
  release_url: string;
}

export type SettingsTarget =
  | "screen_recording"
  | "accessibility"
  | "config_folder"
  | "latest_release";

export type ThemePreference = "system" | "light" | "dark";
