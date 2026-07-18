#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_os = "macos"))]
compile_error!("Ctx UI is supported only on macOS");

use std::sync::{Arc, Mutex};

use ctx_core::{
    AddWindowsReport, CreateWorkspaceReport, CtxApp, CtxAppError, CtxOverview,
    DeleteWorkspacesReport, SwitchReport, UrlLaunchReport, WindowPickerOverview,
};
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, Rect, State, WebviewWindow, WindowEvent,
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

const TRAY_ID: &str = "ctx";
const POPOVER_LABEL: &str = "popover";
const POPOVER_GAP: f64 = 6.0;

#[derive(Clone)]
struct AppState {
    core: CtxApp,
    operation_gate: Arc<Mutex<()>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct CommandError {
    code: String,
    message: String,
}

impl CommandError {
    fn internal(message: impl Into<String>) -> Self {
        Self {
            code: "internal".to_string(),
            message: message.into(),
        }
    }
}

impl From<CtxAppError> for CommandError {
    fn from(error: CtxAppError) -> Self {
        Self {
            code: error.code().to_string(),
            message: error.to_string(),
        }
    }
}

async fn run_core<T, F>(state: AppState, operation: F) -> Result<T, CommandError>
where
    T: Send + 'static,
    F: FnOnce(&CtxApp) -> Result<T, CtxAppError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || run_core_blocking(state, operation))
        .await
        .map_err(|error| CommandError::internal(format!("Ctx worker failed: {error}")))?
}

fn run_core_blocking<T, F>(state: AppState, operation: F) -> Result<T, CommandError>
where
    F: FnOnce(&CtxApp) -> Result<T, CtxAppError>,
{
    let _guard = state
        .operation_gate
        .lock()
        .map_err(|_| CommandError::internal("Ctx operation lock is poisoned"))?;
    operation(&state.core).map_err(CommandError::from)
}

#[tauri::command]
async fn get_overview(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CtxOverview, CommandError> {
    let overview = run_core(state.inner().clone(), CtxApp::overview).await?;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let active = overview
            .active_workspace
            .as_deref()
            .unwrap_or("No active workspace");
        let _ = tray.set_tooltip(Some(format!("Ctx — {active}")));
    }
    Ok(overview)
}

#[tauri::command]
async fn switch_workspace(
    name: String,
    state: State<'_, AppState>,
) -> Result<SwitchReport, CommandError> {
    run_core(state.inner().clone(), move |core| {
        core.switch_workspace(&name)
    })
    .await
}

#[tauri::command]
async fn open_workspace_urls(
    name: String,
    state: State<'_, AppState>,
) -> Result<UrlLaunchReport, CommandError> {
    run_core(state.inner().clone(), move |core| {
        core.open_workspace_urls(&name, true)
    })
    .await
}

#[tauri::command]
async fn get_window_candidates(
    workspace: String,
    state: State<'_, AppState>,
) -> Result<WindowPickerOverview, CommandError> {
    run_core(state.inner().clone(), move |core| {
        core.window_candidates(&workspace)
    })
    .await
}

#[tauri::command]
async fn add_windows_to_workspace(
    workspace: String,
    window_ids: Vec<u32>,
    state: State<'_, AppState>,
) -> Result<AddWindowsReport, CommandError> {
    run_core(state.inner().clone(), move |core| {
        core.add_windows_to_workspace(&workspace, &window_ids)
    })
    .await
}

#[tauri::command]
async fn create_workspace(
    name: String,
    state: State<'_, AppState>,
) -> Result<CreateWorkspaceReport, CommandError> {
    run_core(state.inner().clone(), move |core| {
        core.create_workspace(&name)
    })
    .await
}

#[tauri::command]
async fn delete_workspace(
    name: String,
    state: State<'_, AppState>,
) -> Result<DeleteWorkspacesReport, CommandError> {
    run_core(state.inner().clone(), move |core| {
        core.delete_workspace(&name)
    })
    .await
}

#[tauri::command]
async fn delete_all_workspaces(
    state: State<'_, AppState>,
) -> Result<DeleteWorkspacesReport, CommandError> {
    run_core(state.inner().clone(), CtxApp::delete_all_workspaces).await
}

#[tauri::command]
fn hide_popover(app: AppHandle) -> Result<(), CommandError> {
    popover(&app)?.hide().map_err(window_error)
}

#[tauri::command]
fn show_popover(app: AppHandle) -> Result<(), CommandError> {
    reveal_current_popover(&app)
}

fn reveal_current_popover(app: &AppHandle) -> Result<(), CommandError> {
    let rect = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| CommandError::internal("Ctx tray icon is unavailable"))?
        .rect()
        .map_err(window_error)?
        .ok_or_else(|| CommandError::internal("Ctx tray position is unavailable"))?;
    reveal_popover(app, &rect)
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

fn window_error(error: impl ToString) -> CommandError {
    CommandError::internal(error.to_string())
}

fn popover(app: &AppHandle) -> Result<WebviewWindow, CommandError> {
    app.get_webview_window(POPOVER_LABEL)
        .ok_or_else(|| CommandError::internal("Ctx popover is unavailable"))
}

fn reveal_popover(app: &AppHandle, tray_rect: &Rect) -> Result<(), CommandError> {
    let window = popover(app)?;
    position_popover(app, &window, tray_rect)?;
    window.show().map_err(window_error)?;
    window.set_focus().map_err(window_error)?;
    app.emit_to(POPOVER_LABEL, "ctx://popover-opened", ())
        .map_err(window_error)
}

fn position_popover(
    app: &AppHandle,
    window: &WebviewWindow,
    tray_rect: &Rect,
) -> Result<(), CommandError> {
    let scale = window.scale_factor().map_err(window_error)?;
    let tray_position = tray_rect.position.to_physical::<i32>(scale);
    let tray_size = tray_rect.size.to_physical::<u32>(scale);
    let window_size = window.outer_size().map_err(window_error)?;
    let center_x = f64::from(tray_position.x) + f64::from(tray_size.width) / 2.0;
    let monitor = app
        .monitor_from_point(center_x, f64::from(tray_position.y))
        .map_err(window_error)?
        .or_else(|| app.primary_monitor().ok().flatten())
        .ok_or_else(|| CommandError::internal("No monitor is available for the Ctx popover"))?;
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let position = clamp_popover_position(
        PhysicalBounds {
            x: f64::from(tray_position.x),
            y: f64::from(tray_position.y),
            width: f64::from(tray_size.width),
            height: f64::from(tray_size.height),
        },
        PhysicalBounds {
            x: f64::from(monitor_position.x),
            y: f64::from(monitor_position.y),
            width: f64::from(monitor_size.width),
            height: f64::from(monitor_size.height),
        },
        f64::from(window_size.width),
        f64::from(window_size.height),
        POPOVER_GAP * scale,
    );
    window
        .set_position(PhysicalPosition::new(position.0, position.1))
        .map_err(window_error)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PhysicalBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn clamp_popover_position(
    tray: PhysicalBounds,
    monitor: PhysicalBounds,
    popover_width: f64,
    popover_height: f64,
    gap: f64,
) -> (i32, i32) {
    let desired_x = tray.x + (tray.width - popover_width) / 2.0;
    let desired_y = tray.y + tray.height + gap;
    let max_x = (monitor.x + monitor.width - popover_width).max(monitor.x);
    let max_y = (monitor.y + monitor.height - popover_height).max(monitor.y);
    (
        desired_x.clamp(monitor.x, max_x).round() as i32,
        desired_y.clamp(monitor.y, max_y).round() as i32,
    )
}

fn template_tray_icon() -> Image<'static> {
    const WIDTH: u32 = 18;
    const HEIGHT: u32 = 18;
    let mut rgba = vec![0_u8; (WIDTH * HEIGHT * 4) as usize];
    for y in 3..15 {
        for x in 3..15 {
            let c_stroke = x <= 5 || y <= 5 || y >= 12;
            let crossbar = (8..=10).contains(&y) && x >= 8;
            if c_stroke || crossbar {
                let index = ((y * WIDTH + x) * 4) as usize;
                rgba[index..index + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }
    Image::new_owned(rgba, WIDTH, HEIGHT)
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            core: CtxApp::discover(None).expect("failed to resolve Ctx application paths"),
            operation_gate: Arc::new(Mutex::new(())),
        })
        .setup(|app| {
            app.handle()
                .set_activation_policy(tauri::ActivationPolicy::Accessory)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit Ctx", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit_item])?;
            TrayIconBuilder::with_id(TRAY_ID)
                .icon(template_tray_icon())
                .icon_as_template(true)
                .tooltip("Ctx — No active workspace")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        rect,
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Ok(window) = popover(app) {
                            match popover_toggle_action(window.is_visible().unwrap_or(false)) {
                                PopoverToggleAction::Hide => {
                                    let _ = window.hide();
                                }
                                PopoverToggleAction::Show => {
                                    let _ = reveal_popover(app, &rect);
                                }
                            }
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == POPOVER_LABEL && matches!(event, WindowEvent::Focused(false)) {
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_overview,
            switch_workspace,
            open_workspace_urls,
            get_window_candidates,
            add_windows_to_workspace,
            create_workspace,
            delete_workspace,
            delete_all_workspaces,
            hide_popover,
            show_popover,
            quit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Ctx");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PopoverToggleAction {
    Show,
    Hide,
}

fn popover_toggle_action(is_visible: bool) -> PopoverToggleAction {
    if is_visible {
        PopoverToggleAction::Hide
    } else {
        PopoverToggleAction::Show
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::mpsc::{self, TryRecvError},
        thread,
        time::Duration,
    };

    use super::*;

    #[test]
    fn popover_is_centered_below_the_tray() {
        let result = clamp_popover_position(
            PhysicalBounds {
                x: 900.0,
                y: 0.0,
                width: 24.0,
                height: 24.0,
            },
            PhysicalBounds {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
            400.0,
            560.0,
            6.0,
        );

        assert_eq!(result, (712, 30));
    }

    #[test]
    fn popover_is_clamped_to_monitor_edges() {
        let right = clamp_popover_position(
            PhysicalBounds {
                x: 2500.0,
                y: 0.0,
                width: 24.0,
                height: 24.0,
            },
            PhysicalBounds {
                x: 1920.0,
                y: 0.0,
                width: 640.0,
                height: 480.0,
            },
            400.0,
            560.0,
            6.0,
        );

        assert_eq!(right, (2160, 0));
    }

    #[test]
    fn core_errors_serialize_with_stable_code_and_message() {
        let error = CommandError::from(CtxAppError::WorkspaceMissing {
            name: "missing".to_string(),
        });

        assert_eq!(error.code, "workspace_missing");
        assert!(error.message.contains("missing"));
    }

    #[test]
    fn tray_click_toggles_the_popover_visibility() {
        assert_eq!(popover_toggle_action(false), PopoverToggleAction::Show);
        assert_eq!(popover_toggle_action(true), PopoverToggleAction::Hide);
    }

    #[test]
    fn operation_gate_serializes_core_commands() {
        let gate = Arc::new(Mutex::new(()));
        let state = AppState {
            core: CtxApp::from_paths("/tmp/config.yaml", "/tmp/runtime.json"),
            operation_gate: gate.clone(),
        };
        let guard = gate.lock().unwrap();
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            run_core_blocking(state, |_| {
                sender.send(()).unwrap();
                Ok(())
            })
        });

        assert_eq!(receiver.try_recv(), Err(TryRecvError::Empty));
        drop(guard);
        receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        worker.join().unwrap().unwrap();
    }
}
