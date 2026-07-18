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

export interface CommandError {
  code: string;
  message: string;
}
