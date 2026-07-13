use serde::Serialize;
use thiserror::Error;

use crate::DesktopPlacement;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpaceInventory {
    pub displays: Vec<DisplaySpaces>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DisplaySpaces {
    pub uuid: String,
    pub current_space_id: u64,
    pub desktops: Vec<DesktopSpace>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DesktopSpace {
    pub id: u64,
    pub ordinal: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WindowPlacement {
    pub window_id: u32,
    pub display_uuid: String,
    pub space_id: u64,
    pub desktop_ordinal: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementChange {
    AlreadyPlaced,
    Moved,
}

#[derive(Debug, Error)]
pub enum SpaceError {
    #[error("macOS Desktop inspection is only supported on macOS")]
    UnsupportedPlatform,

    #[error("macOS Desktop inspection is unavailable: {0}")]
    ApiUnavailable(String),

    #[error("macOS returned invalid Desktop metadata: {0}")]
    InvalidMetadata(String),

    #[error("display '{display_uuid}' is unavailable")]
    DisplayUnavailable { display_uuid: String },

    #[error("display '{display_uuid}' has no Desktop {desktop_ordinal}")]
    DesktopUnavailable {
        display_uuid: String,
        desktop_ordinal: usize,
    },
}

#[cfg(target_os = "macos")]
pub fn list_spaces() -> Result<SpaceInventory, SpaceError> {
    macos::list_spaces()
}

#[cfg(target_os = "macos")]
pub fn window_placement(window_id: u32) -> Result<WindowPlacement, SpaceError> {
    macos::window_placement(window_id)
}

#[cfg(not(target_os = "macos"))]
pub fn window_placement(_window_id: u32) -> Result<WindowPlacement, SpaceError> {
    Err(SpaceError::UnsupportedPlatform)
}

pub fn capture_desktop_placement(window_id: u32) -> Result<DesktopPlacement, SpaceError> {
    let placement = window_placement(window_id)?;
    Ok(DesktopPlacement {
        display_uuid: placement.display_uuid,
        desktop_ordinal: placement.desktop_ordinal,
    })
}

pub fn current_desktop_placement(display_uuid: &str) -> Result<DesktopPlacement, SpaceError> {
    let inventory = list_spaces()?;
    let display = inventory
        .displays
        .iter()
        .find(|display| display.uuid == display_uuid)
        .ok_or_else(|| SpaceError::DisplayUnavailable {
            display_uuid: display_uuid.to_string(),
        })?;
    let desktop = display
        .desktops
        .iter()
        .find(|desktop| desktop.id == display.current_space_id)
        .ok_or_else(|| {
            SpaceError::InvalidMetadata(format!(
                "display '{display_uuid}' has no current user Desktop"
            ))
        })?;
    Ok(DesktopPlacement {
        display_uuid: display_uuid.to_string(),
        desktop_ordinal: desktop.ordinal,
    })
}

#[cfg(target_os = "macos")]
pub fn move_window_to_desktop(
    window_id: u32,
    placement: &DesktopPlacement,
) -> Result<PlacementChange, SpaceError> {
    macos::move_window_to_desktop(window_id, placement)
}

#[cfg(not(target_os = "macos"))]
pub fn move_window_to_desktop(
    _window_id: u32,
    _placement: &DesktopPlacement,
) -> Result<PlacementChange, SpaceError> {
    Err(SpaceError::UnsupportedPlatform)
}

#[cfg(not(target_os = "macos"))]
pub fn list_spaces() -> Result<SpaceInventory, SpaceError> {
    Err(SpaceError::UnsupportedPlatform)
}

#[cfg(target_os = "macos")]
#[allow(deprecated, unexpected_cfgs)]
mod macos {
    use std::{
        ffi::{CStr, c_char, c_int, c_void},
        mem,
        process::Command,
        ptr,
        sync::{Mutex, MutexGuard, Once},
        thread,
        time::{Duration, Instant},
    };

    use accessibility::{
        action::AXUIElementActions,
        attribute::{AXAttribute, AXUIElementAttributes},
        ui_element::AXUIElement,
    };
    use core_foundation::{
        array::CFArray,
        base::{CFType, TCFType},
        number::CFNumber,
        runloop::CFRunLoop,
        string::CFString,
    };
    use core_foundation_sys::{
        array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef},
        base::CFRelease,
        dictionary::{CFDictionaryGetValue, CFDictionaryRef},
        number::{CFNumberGetValue, CFNumberRef, kCFNumberSInt64Type},
        runloop::kCFRunLoopDefaultMode,
        string::CFStringRef,
        uuid::{CFUUIDCreateFromString, CFUUIDRef},
    };

    use super::{
        DesktopPlacement, DesktopSpace, DisplaySpaces, PlacementChange, SpaceError, SpaceInventory,
        WindowPlacement,
    };

    const RTLD_LAZY: c_int = 0x1;
    const RTLD_LOCAL: c_int = 0x4;
    const SKYLIGHT_PATH: &CStr = c"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight";
    const SKYLIGHT_IMAGE_PATH: &CStr =
        c"/System/Library/PrivateFrameworks/SkyLight.framework/Versions/A/SkyLight";
    const BRIDGED_MOVE_SYMBOL: &CStr = c"__ZL54SLSPerformAsynchronousBridgedWindowManagementOperationP47SLSAsynchronousBridgedWindowManagementOperation";
    static SKYLIGHT_LOCK: Mutex<()> = Mutex::new(());
    static APPKIT_INIT: Once = Once::new();

    type MainConnectionId = unsafe extern "C" fn() -> c_int;
    type CopyManagedDisplaySpaces = unsafe extern "C" fn(c_int) -> CFArrayRef;
    type ManagedDisplayGetCurrentSpace = unsafe extern "C" fn(c_int, CFStringRef) -> u64;
    type SpaceGetType = unsafe extern "C" fn(c_int, u64) -> c_int;
    type CopySpacesForWindows = unsafe extern "C" fn(c_int, c_int, CFArrayRef) -> CFArrayRef;
    type MoveWindowsToManagedSpace = unsafe extern "C" fn(c_int, CFArrayRef, u64);
    type PerformBridgedWindowManagementOperation =
        unsafe extern "C" fn(*mut objc::runtime::Object) -> i64;
    type SpaceSetCompatId = unsafe extern "C" fn(c_int, u64, c_int) -> c_int;
    type SetWindowListWorkspace = unsafe extern "C" fn(c_int, *const u32, c_int, c_int) -> c_int;
    type CoreDockSendNotification = unsafe extern "C" fn(CFStringRef, c_int) -> c_int;
    type DisplayGetIdFromUuid = unsafe extern "C" fn(CFUUIDRef) -> u32;

    unsafe extern "C" {
        fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlerror() -> *const c_char;
        fn _dyld_image_count() -> u32;
        fn _dyld_get_image_name(index: u32) -> *const c_char;
        fn _dyld_get_image_vmaddr_slide(index: u32) -> isize;
        fn _dyld_get_image_header(index: u32) -> *const MachHeader64;
    }

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {
        fn NSApplicationLoad() -> bool;
    }

    #[repr(C)]
    struct MachHeader64 {
        magic: u32,
        cpu_type: i32,
        cpu_subtype: i32,
        file_type: u32,
        command_count: u32,
        command_size: u32,
        flags: u32,
        reserved: u32,
    }

    #[repr(C)]
    struct LoadCommand {
        command: u32,
        size: u32,
    }

    #[repr(C)]
    struct SegmentCommand64 {
        command: u32,
        size: u32,
        name: [c_char; 16],
        address: u64,
        memory_size: u64,
        file_offset: u64,
        file_size: u64,
        maximum_protection: i32,
        initial_protection: i32,
        section_count: u32,
        flags: u32,
    }

    #[repr(C)]
    struct SymbolTableCommand {
        command: u32,
        size: u32,
        symbol_offset: u32,
        symbol_count: u32,
        string_offset: u32,
        string_size: u32,
    }

    #[repr(C)]
    struct Symbol64 {
        string_index: u32,
        symbol_type: u8,
        section: u8,
        description: u16,
        value: u64,
    }

    struct SkyLight {
        _handle: *mut c_void,
        main_connection_id: MainConnectionId,
        copy_managed_display_spaces: CopyManagedDisplaySpaces,
        managed_display_get_current_space: ManagedDisplayGetCurrentSpace,
        space_get_type: SpaceGetType,
        copy_spaces_for_windows: CopySpacesForWindows,
        move_windows_to_managed_space: Option<MoveWindowsToManagedSpace>,
        perform_bridged_window_management_operation:
            Option<PerformBridgedWindowManagementOperation>,
        space_set_compat_id: Option<SpaceSetCompatId>,
        set_window_list_workspace: Option<SetWindowListWorkspace>,
        core_dock_send_notification: Option<CoreDockSendNotification>,
        display_get_id_from_uuid: Option<DisplayGetIdFromUuid>,
    }

    impl SkyLight {
        fn load() -> Result<Self, SpaceError> {
            unsafe {
                APPKIT_INIT.call_once(|| {
                    let _ = NSApplicationLoad();
                });
                let handle = dlopen(SKYLIGHT_PATH.as_ptr(), RTLD_LAZY | RTLD_LOCAL);
                if handle.is_null() {
                    return Err(SpaceError::ApiUnavailable(last_dl_error()));
                }

                Ok(Self {
                    _handle: handle,
                    main_connection_id: mem::transmute::<*mut c_void, MainConnectionId>(
                        symbol_any(handle, &[c"CGSMainConnectionID", c"SLSMainConnectionID"])?,
                    ),
                    copy_managed_display_spaces: mem::transmute::<
                        *mut c_void,
                        CopyManagedDisplaySpaces,
                    >(symbol_any(
                        handle,
                        &[
                            c"CGSCopyManagedDisplaySpaces",
                            c"SLSCopyManagedDisplaySpaces",
                        ],
                    )?),
                    managed_display_get_current_space: mem::transmute::<
                        *mut c_void,
                        ManagedDisplayGetCurrentSpace,
                    >(symbol_any(
                        handle,
                        &[
                            c"CGSManagedDisplayGetCurrentSpace",
                            c"SLSManagedDisplayGetCurrentSpace",
                        ],
                    )?),
                    space_get_type: mem::transmute::<*mut c_void, SpaceGetType>(symbol_any(
                        handle,
                        &[c"CGSSpaceGetType", c"SLSSpaceGetType"],
                    )?),
                    copy_spaces_for_windows: mem::transmute::<*mut c_void, CopySpacesForWindows>(
                        symbol_any(
                            handle,
                            &[c"CGSCopySpacesForWindows", c"SLSCopySpacesForWindows"],
                        )?,
                    ),
                    move_windows_to_managed_space: symbol_any(
                        handle,
                        &[
                            c"CGSMoveWindowsToManagedSpace",
                            c"SLSMoveWindowsToManagedSpace",
                        ],
                    )
                    .ok()
                    .map(|pointer| {
                        mem::transmute::<*mut c_void, MoveWindowsToManagedSpace>(pointer)
                    }),
                    perform_bridged_window_management_operation: symbol(
                        handle,
                        c"SLSPerformAsynchronousBridgedWindowManagementOperation",
                    )
                    .ok()
                    .or_else(|| macho_find_symbol(SKYLIGHT_IMAGE_PATH, BRIDGED_MOVE_SYMBOL))
                    .map(|pointer| {
                        mem::transmute::<*mut c_void, PerformBridgedWindowManagementOperation>(
                            pointer,
                        )
                    }),
                    space_set_compat_id: symbol_any(
                        handle,
                        &[c"CGSSpaceSetCompatID", c"SLSSpaceSetCompatID"],
                    )
                    .ok()
                    .map(|pointer| mem::transmute::<*mut c_void, SpaceSetCompatId>(pointer)),
                    set_window_list_workspace: symbol_any(
                        handle,
                        &[c"CGSSetWindowListWorkspace", c"SLSSetWindowListWorkspace"],
                    )
                    .ok()
                    .map(|pointer| mem::transmute::<*mut c_void, SetWindowListWorkspace>(pointer)),
                    core_dock_send_notification: symbol(handle, c"CoreDockSendNotification")
                        .ok()
                        .or_else(|| macho_find_symbol_any_image(c"_CoreDockSendNotification"))
                        .map(|pointer| {
                            mem::transmute::<*mut c_void, CoreDockSendNotification>(pointer)
                        }),
                    display_get_id_from_uuid: symbol(handle, c"CGDisplayGetDisplayIDFromUUID")
                        .ok()
                        .map(|pointer| {
                            mem::transmute::<*mut c_void, DisplayGetIdFromUuid>(pointer)
                        }),
                })
            }
        }
    }

    pub(super) fn list_spaces() -> Result<SpaceInventory, SpaceError> {
        let _guard = skylight_lock();
        unsafe {
            let api = SkyLight::load()?;
            let connection = (api.main_connection_id)();
            let raw_displays = (api.copy_managed_display_spaces)(connection);
            if raw_displays.is_null() {
                return plist_inventory();
            }

            let result = parse_displays(&api, connection, raw_displays);
            CFRelease(raw_displays.cast());
            match result {
                Ok(displays) => Ok(SpaceInventory { displays }),
                Err(_) => plist_inventory(),
            }
        }
    }

    pub(super) fn window_placement(window_id: u32) -> Result<WindowPlacement, SpaceError> {
        let inventory = list_spaces()?;
        let space_id =
            skylight_window_space(window_id).or_else(|_| plist_space_id_for_window(window_id))?;
        inventory
            .displays
            .into_iter()
            .find_map(|display| {
                display
                    .desktops
                    .iter()
                    .find(|desktop| desktop.id == space_id)
                    .map(|desktop| WindowPlacement {
                        window_id,
                        display_uuid: display.uuid,
                        space_id,
                        desktop_ordinal: desktop.ordinal,
                    })
            })
            .ok_or_else(|| {
                SpaceError::InvalidMetadata(format!(
                    "window {window_id} belongs to unknown or non-user Space {space_id}"
                ))
            })
    }

    pub(super) fn move_window_to_desktop(
        window_id: u32,
        placement: &DesktopPlacement,
    ) -> Result<PlacementChange, SpaceError> {
        let inventory = list_spaces()?;
        let destination = match resolve_desktop(&inventory, placement) {
            Ok(desktop) => *desktop,
            Err(SpaceError::DesktopUnavailable { .. }) => ensure_desktop(placement)?,
            Err(error) => return Err(error),
        };
        if window_placement(window_id).is_ok_and(|current| current.space_id == destination.id) {
            return Ok(PlacementChange::AlreadyPlaced);
        }

        let mut used_legacy_move = false;
        {
            let _guard = skylight_lock();
            let api = SkyLight::load()?;
            let connection = unsafe { (api.main_connection_id)() };
            let window_number = CFNumber::from(i64::from(window_id));
            let windows = CFArray::from_CFTypes(&[window_number]);
            if let Some(perform) = api.perform_bridged_window_management_operation {
                perform_bridged_window_move(perform, &windows, destination.id)?;
            } else if uses_compat_workspace_move()
                && let (Some(set_compat), Some(set_workspace)) =
                    (api.space_set_compat_id, api.set_window_list_workspace)
            {
                const CTX_COMPAT_ID: c_int = 0x6374_7821;
                unsafe {
                    set_compat(connection, destination.id, CTX_COMPAT_ID);
                    set_workspace(connection, &window_id, 1, CTX_COMPAT_ID);
                    set_compat(connection, destination.id, 0);
                }
            } else if let Some(move_windows) = api.move_windows_to_managed_space {
                used_legacy_move = true;
                unsafe {
                    move_windows(connection, windows.as_concrete_TypeRef(), destination.id);
                }
            } else {
                return Err(SpaceError::ApiUnavailable(
                    "SkyLight does not expose a compatible managed-Space window movement API"
                        .to_string(),
                ));
            }
        }
        if let Err(primary_error) = wait_for_window_space(window_id, destination.id) {
            if used_legacy_move {
                return Err(primary_error);
            }
            let guard = skylight_lock();
            let api = SkyLight::load()?;
            let move_windows = api.move_windows_to_managed_space.ok_or(primary_error)?;
            let connection = unsafe { (api.main_connection_id)() };
            let window_number = CFNumber::from(i64::from(window_id));
            let windows = CFArray::from_CFTypes(&[window_number]);
            unsafe {
                move_windows(connection, windows.as_concrete_TypeRef(), destination.id);
            }
            drop(guard);
            wait_for_window_space(window_id, destination.id)?;
        }
        Ok(PlacementChange::Moved)
    }

    fn perform_bridged_window_move(
        perform: PerformBridgedWindowManagementOperation,
        windows: &CFArray<CFNumber>,
        destination_space_id: u64,
    ) -> Result<(), SpaceError> {
        use objc::{msg_send, runtime::Class, sel, sel_impl};

        let class =
            Class::get("SLSBridgedMoveWindowsToManagedSpaceOperation").ok_or_else(|| {
                SpaceError::ApiUnavailable(
                    "SkyLight bridged window movement class is unavailable".to_string(),
                )
            })?;
        unsafe {
            let operation: *mut objc::runtime::Object = msg_send![class, alloc];
            if operation.is_null() {
                return Err(SpaceError::ApiUnavailable(
                    "could not allocate a bridged window movement operation".to_string(),
                ));
            }
            let operation: *mut objc::runtime::Object = msg_send![
                operation,
                initWithWindows: windows.as_concrete_TypeRef()
                spaceID: destination_space_id
            ];
            if operation.is_null() {
                return Err(SpaceError::ApiUnavailable(
                    "could not initialize a bridged window movement operation".to_string(),
                ));
            }
            perform(operation);
            let _: () = msg_send![operation, release];
        }
        Ok(())
    }

    fn wait_for_window_space(window_id: u32, expected_space_id: u64) -> Result<(), SpaceError> {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if skylight_window_space(window_id).is_ok_and(|space_id| space_id == expected_space_id)
            {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(SpaceError::InvalidMetadata(format!(
                    "window {window_id} did not move to Space {expected_space_id}"
                )));
            }
            CFRunLoop::run_in_mode(
                unsafe { kCFRunLoopDefaultMode },
                Duration::from_millis(50),
                true,
            );
        }
    }

    fn uses_compat_workspace_move() -> bool {
        Command::new("/usr/bin/sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .is_some_and(|version| version_requires_compat_move(version.trim()))
    }

    pub(super) fn version_requires_compat_move(version: &str) -> bool {
        let mut parts = version
            .split('.')
            .filter_map(|part| part.parse::<u32>().ok());
        let major = parts.next().unwrap_or_default();
        let minor = parts.next().unwrap_or_default();
        major > 14 || (major == 14 && minor >= 5)
    }

    fn ensure_desktop(placement: &DesktopPlacement) -> Result<DesktopSpace, SpaceError> {
        let mut inventory = list_spaces()?;
        let existing_count = inventory
            .displays
            .iter()
            .find(|display| display.uuid == placement.display_uuid)
            .ok_or_else(|| SpaceError::DisplayUnavailable {
                display_uuid: placement.display_uuid.clone(),
            })?
            .desktops
            .len();
        let missing = missing_desktop_count(existing_count, placement.desktop_ordinal);

        for _ in 0..missing {
            create_desktop(&placement.display_uuid)?;
            let previous_count = inventory
                .displays
                .iter()
                .find(|display| display.uuid == placement.display_uuid)
                .map_or(0, |display| display.desktops.len());
            inventory = wait_for_desktop_count(&placement.display_uuid, previous_count + 1)?;
        }

        resolve_desktop(&inventory, placement).copied()
    }

    fn wait_for_desktop_count(
        display_uuid: &str,
        expected: usize,
    ) -> Result<SpaceInventory, SpaceError> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let inventory = list_spaces()?;
            let count = inventory
                .displays
                .iter()
                .find(|display| display.uuid == display_uuid)
                .map_or(0, |display| display.desktops.len());
            if count >= expected {
                return Ok(inventory);
            }
            if Instant::now() >= deadline {
                return Err(SpaceError::InvalidMetadata(format!(
                    "Desktop creation did not add Desktop {expected} on display '{display_uuid}'"
                )));
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn create_desktop(display_uuid: &str) -> Result<(), SpaceError> {
        if !crate::request_accessibility_permission() {
            return Err(SpaceError::ApiUnavailable(
                "Accessibility permission is required to create a macOS Desktop".to_string(),
            ));
        }
        let api = SkyLight::load()?;
        let notify = api.core_dock_send_notification.ok_or_else(|| {
            SpaceError::ApiUnavailable(
                "CoreDock Mission Control notification is unavailable".to_string(),
            )
        })?;
        let display_id = display_id(&api, display_uuid)?;
        let mission_control = CFString::new("com.apple.expose.awake");
        unsafe {
            notify(mission_control.as_concrete_TypeRef(), 0);
        }

        let result = (|| {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if let Ok(dock) = AXUIElement::application_with_bundle("com.apple.dock")
                    && let Some(display) = find_element(&dock, 0, &|element| {
                        element_identifier(element).as_deref() == Some("mc.display")
                            && element_display_id(element) == Some(display_id)
                    })
                    && let Some(add) = find_element(&display, 0, &|element| {
                        element_identifier(element).as_deref() == Some("mc.spaces.add")
                    })
                {
                    return add.press().map_err(|error| {
                        SpaceError::ApiUnavailable(format!(
                            "could not press the Mission Control add-Desktop control: {error}"
                        ))
                    });
                }
                if Instant::now() >= deadline {
                    return Err(SpaceError::ApiUnavailable(format!(
                        "Mission Control did not expose an add-Desktop control for display '{display_uuid}'"
                    )));
                }
                thread::sleep(Duration::from_millis(100));
            }
        })();

        unsafe {
            notify(mission_control.as_concrete_TypeRef(), 0);
        }
        result
    }

    fn display_id(api: &SkyLight, display_uuid: &str) -> Result<u32, SpaceError> {
        if display_uuid == "Main" {
            return Ok(core_graphics::display::CGDisplay::main().id);
        }
        let get_id = api.display_get_id_from_uuid.ok_or_else(|| {
            SpaceError::ApiUnavailable(
                "CoreGraphics display UUID resolution is unavailable".to_string(),
            )
        })?;
        let value = CFString::new(display_uuid);
        let uuid = unsafe { CFUUIDCreateFromString(ptr::null(), value.as_concrete_TypeRef()) };
        if uuid.is_null() {
            return Err(SpaceError::InvalidMetadata(format!(
                "display identifier '{display_uuid}' is not a UUID"
            )));
        }
        let id = unsafe { get_id(uuid) };
        unsafe { CFRelease(uuid.cast()) };
        if id == 0 {
            Err(SpaceError::DisplayUnavailable {
                display_uuid: display_uuid.to_string(),
            })
        } else {
            Ok(id)
        }
    }

    fn find_element(
        element: &AXUIElement,
        depth: usize,
        predicate: &impl Fn(&AXUIElement) -> bool,
    ) -> Option<AXUIElement> {
        if predicate(element) {
            return Some(element.clone());
        }
        if depth >= 8 {
            return None;
        }
        element
            .children()
            .ok()?
            .iter()
            .find_map(|child| find_element(&child, depth + 1, predicate))
    }

    fn element_identifier(element: &AXUIElement) -> Option<String> {
        element.identifier().ok().map(|value| value.to_string())
    }

    fn element_display_id(element: &AXUIElement) -> Option<u32> {
        let attribute = AXAttribute::<CFType>::new(&CFString::new("AXDisplayID"));
        element
            .attribute(&attribute)
            .ok()?
            .downcast::<CFNumber>()?
            .to_i64()?
            .try_into()
            .ok()
    }

    fn skylight_window_space(window_id: u32) -> Result<u64, SpaceError> {
        let _guard = skylight_lock();
        let api = SkyLight::load()?;
        let connection = unsafe { (api.main_connection_id)() };
        let window_number = CFNumber::from(i64::from(window_id));
        let windows = CFArray::from_CFTypes(&[window_number]);
        let spaces = unsafe {
            (api.copy_spaces_for_windows)(connection, 0x7, windows.as_concrete_TypeRef())
        };
        if spaces.is_null() {
            return Err(SpaceError::InvalidMetadata(format!(
                "window {window_id} has no Space membership"
            )));
        }
        let id = if unsafe { CFArrayGetCount(spaces) } == 0 {
            None
        } else {
            let number = unsafe { CFArrayGetValueAtIndex(spaces, 0) as CFNumberRef };
            number_value(number)
        };
        unsafe { CFRelease(spaces.cast()) };
        id.ok_or_else(|| {
            SpaceError::InvalidMetadata(format!("window {window_id} has no Space membership"))
        })
    }

    fn skylight_lock() -> MutexGuard<'static, ()> {
        SKYLIGHT_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn plist_inventory() -> Result<SpaceInventory, SpaceError> {
        let (root, path) = read_spaces_plist()?;
        let displays = displays_from_plist(&root)?;
        if displays.is_empty() {
            Err(SpaceError::InvalidMetadata(format!(
                "no active displays were found in {}",
                path.display()
            )))
        } else {
            Ok(SpaceInventory { displays })
        }
    }

    fn read_spaces_plist() -> Result<(serde_json::Value, std::path::PathBuf), SpaceError> {
        let home = directories::BaseDirs::new()
            .ok_or_else(|| SpaceError::InvalidMetadata("home directory is unavailable".into()))?;
        let path = home
            .home_dir()
            .join("Library/Preferences/com.apple.spaces.plist");
        let output = Command::new("/usr/bin/plutil")
            .args(["-convert", "json", "-o", "-"])
            .arg(&path)
            .output()
            .map_err(|error| {
                SpaceError::InvalidMetadata(format!("could not read {}: {error}", path.display()))
            })?;
        if !output.status.success() {
            return Err(SpaceError::InvalidMetadata(format!(
                "could not convert {}: {}",
                path.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let root = serde_json::from_slice(&output.stdout).map_err(|error| {
            SpaceError::InvalidMetadata(format!("could not parse {}: {error}", path.display()))
        })?;
        Ok((root, path))
    }

    fn displays_from_plist(root: &serde_json::Value) -> Result<Vec<DisplaySpaces>, SpaceError> {
        let monitors = root
            .pointer("/SpacesDisplayConfiguration/Management Data/Monitors")
            .and_then(|value| value.as_array())
            .ok_or_else(|| {
                SpaceError::InvalidMetadata("Space monitor inventory is missing".into())
            })?;
        let mut displays = Vec::new();
        for monitor in monitors {
            let Some(spaces) = monitor.get("Spaces").and_then(|value| value.as_array()) else {
                continue;
            };
            let Some(uuid) = monitor
                .get("Display Identifier")
                .and_then(|value| value.as_str())
            else {
                continue;
            };
            let current_space_id = monitor
                .pointer("/Current Space/id64")
                .and_then(|value| value.as_u64())
                .unwrap_or_default();
            let desktops = normalize_desktops(spaces.iter().filter_map(|space| {
                Some((
                    space.get("id64")?.as_u64()?,
                    i32::try_from(space.get("type")?.as_i64()?).ok()?,
                ))
            }));
            displays.push(DisplaySpaces {
                uuid: uuid.to_string(),
                current_space_id,
                desktops,
            });
        }
        Ok(displays)
    }

    fn plist_space_id_for_window(window_id: u32) -> Result<u64, SpaceError> {
        let (root, _) = read_spaces_plist()?;
        let displays = displays_from_plist(&root)?;
        let properties = root
            .pointer("/SpacesDisplayConfiguration/Space Properties")
            .and_then(|value| value.as_array())
            .ok_or_else(|| SpaceError::InvalidMetadata("Space properties are missing".into()))?;
        let space_name = properties.iter().find_map(|property| {
            property
                .get("windows")
                .and_then(|value| value.as_array())
                .filter(|windows| {
                    windows
                        .iter()
                        .any(|value| value.as_u64() == Some(u64::from(window_id)))
                })
                .and_then(|_| property.get("name"))
                .and_then(|value| value.as_str())
        });
        let Some(space_name) = space_name else {
            return Err(SpaceError::InvalidMetadata(format!(
                "window {window_id} is not present in the Space preferences"
            )));
        };
        let monitors = root
            .pointer("/SpacesDisplayConfiguration/Management Data/Monitors")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten();
        for monitor in monitors {
            let spaces = monitor
                .get("Spaces")
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten();
            for space in spaces {
                if space.get("uuid").and_then(|value| value.as_str()) == Some(space_name)
                    && let Some(id) = space.get("id64").and_then(|value| value.as_u64())
                    && displays
                        .iter()
                        .any(|display| display.desktops.iter().any(|desktop| desktop.id == id))
                {
                    return Ok(id);
                }
            }
        }
        Err(SpaceError::InvalidMetadata(format!(
            "window {window_id} belongs to an unknown Space"
        )))
    }

    fn parse_displays(
        api: &SkyLight,
        connection: c_int,
        displays: CFArrayRef,
    ) -> Result<Vec<DisplaySpaces>, SpaceError> {
        let display_identifier_key = CFString::new("Display Identifier");
        let spaces_key = CFString::new("Spaces");
        let space_id_key = CFString::new("id64");
        let mut result = Vec::new();

        let count = unsafe { CFArrayGetCount(displays) };
        for index in 0..count {
            let dictionary = unsafe { CFArrayGetValueAtIndex(displays, index) as CFDictionaryRef };
            if dictionary.is_null() {
                continue;
            }
            let identifier = dictionary_value(dictionary, &display_identifier_key) as CFStringRef;
            let spaces = dictionary_value(dictionary, &spaces_key) as CFArrayRef;
            if identifier.is_null() || spaces.is_null() {
                continue;
            }

            let uuid = unsafe { CFString::wrap_under_get_rule(identifier) }.to_string();
            let current_space_id =
                unsafe { (api.managed_display_get_current_space)(connection, identifier) };
            let mut raw_spaces = Vec::new();
            let space_count = unsafe { CFArrayGetCount(spaces) };
            for space_index in 0..space_count {
                let space =
                    unsafe { CFArrayGetValueAtIndex(spaces, space_index) as CFDictionaryRef };
                if space.is_null() {
                    continue;
                }
                let number = dictionary_value(space, &space_id_key) as CFNumberRef;
                let Some(id) = number_value(number) else {
                    continue;
                };
                raw_spaces.push((id, unsafe { (api.space_get_type)(connection, id) }));
            }
            result.push(DisplaySpaces {
                uuid,
                current_space_id,
                desktops: normalize_desktops(raw_spaces),
            });
        }

        if result.is_empty() {
            Err(SpaceError::InvalidMetadata(
                "no managed displays were returned".to_string(),
            ))
        } else {
            Ok(result)
        }
    }

    fn dictionary_value(dictionary: CFDictionaryRef, key: &CFString) -> *const c_void {
        unsafe { CFDictionaryGetValue(dictionary, key.as_CFTypeRef()) }
    }

    fn number_value(number: CFNumberRef) -> Option<u64> {
        if number.is_null() {
            return None;
        }
        let mut value = 0_i64;
        let valid = unsafe {
            CFNumberGetValue(
                number,
                kCFNumberSInt64Type,
                ptr::from_mut(&mut value).cast(),
            )
        };
        (valid && value >= 0).then_some(value as u64)
    }

    pub(super) fn normalize_desktops(
        spaces: impl IntoIterator<Item = (u64, i32)>,
    ) -> Vec<DesktopSpace> {
        spaces
            .into_iter()
            .filter(|(_, space_type)| *space_type == 0)
            .enumerate()
            .map(|(index, (id, _))| DesktopSpace {
                id,
                ordinal: index + 1,
            })
            .collect()
    }

    pub(super) fn resolve_desktop<'a>(
        inventory: &'a SpaceInventory,
        placement: &DesktopPlacement,
    ) -> Result<&'a DesktopSpace, SpaceError> {
        let display = inventory
            .displays
            .iter()
            .find(|display| display.uuid == placement.display_uuid)
            .ok_or_else(|| SpaceError::DisplayUnavailable {
                display_uuid: placement.display_uuid.clone(),
            })?;
        display
            .desktops
            .iter()
            .find(|desktop| desktop.ordinal == placement.desktop_ordinal)
            .ok_or_else(|| SpaceError::DesktopUnavailable {
                display_uuid: placement.display_uuid.clone(),
                desktop_ordinal: placement.desktop_ordinal,
            })
    }

    pub(super) fn missing_desktop_count(existing: usize, target_ordinal: usize) -> usize {
        target_ordinal.saturating_sub(existing)
    }

    unsafe fn macho_find_symbol(image_path: &CStr, symbol_name: &CStr) -> Option<*mut c_void> {
        for index in 0..unsafe { _dyld_image_count() } {
            let name = unsafe { _dyld_get_image_name(index) };
            if !name.is_null() && unsafe { CStr::from_ptr(name) } == image_path {
                return unsafe { macho_find_symbol_in_image(index, symbol_name) };
            }
        }
        None
    }

    unsafe fn macho_find_symbol_any_image(symbol_name: &CStr) -> Option<*mut c_void> {
        for index in 0..unsafe { _dyld_image_count() } {
            if let Some(pointer) = unsafe { macho_find_symbol_in_image(index, symbol_name) } {
                return Some(pointer);
            }
        }
        None
    }

    unsafe fn macho_find_symbol_in_image(
        image_index: u32,
        symbol_name: &CStr,
    ) -> Option<*mut c_void> {
        const LC_SYMTAB: u32 = 0x2;
        const LC_SEGMENT_64: u32 = 0x19;

        let header = unsafe { _dyld_get_image_header(image_index) };
        let slide = unsafe { _dyld_get_image_vmaddr_slide(image_index) };
        if header.is_null() {
            return None;
        }

        let mut linkedit = None;
        let mut symbol_table = None;
        let mut command_address = header.cast::<u8>().addr() + mem::size_of::<MachHeader64>();
        for _ in 0..unsafe { (*header).command_count } {
            let command = command_address as *const LoadCommand;
            if command.is_null()
                || unsafe { (*command).size } < mem::size_of::<LoadCommand>() as u32
            {
                return None;
            }
            match unsafe { (*command).command } {
                LC_SEGMENT_64 => {
                    let segment = command.cast::<SegmentCommand64>();
                    let name = unsafe { &(*segment).name };
                    let bytes = name.map(|value| value as u8);
                    if bytes.starts_with(b"__LINKEDIT\0") {
                        linkedit = Some(segment);
                    }
                }
                LC_SYMTAB => symbol_table = Some(command.cast::<SymbolTableCommand>()),
                _ => {}
            }
            command_address = command_address.checked_add(unsafe { (*command).size } as usize)?;
        }

        let linkedit = unsafe { &*linkedit? };
        let symbol_table = unsafe { &*symbol_table? };
        let linkedit_base = linkedit.address.checked_sub(linkedit.file_offset)? as usize;
        let slid_base = linkedit_base.checked_add_signed(slide)?;
        let strings = slid_base.checked_add(symbol_table.string_offset as usize)? as *const c_char;
        let symbols =
            slid_base.checked_add(symbol_table.symbol_offset as usize)? as *const Symbol64;

        for index in 0..symbol_table.symbol_count as usize {
            let entry = unsafe { &*symbols.add(index) };
            if entry.string_index >= symbol_table.string_size {
                continue;
            }
            let name = unsafe { CStr::from_ptr(strings.add(entry.string_index as usize)) };
            if name == symbol_name {
                let address = (entry.value as usize).checked_add_signed(slide)?;
                return Some(address as *mut c_void);
            }
        }
        None
    }

    unsafe fn symbol(handle: *mut c_void, name: &CStr) -> Result<*mut c_void, SpaceError> {
        let pointer = unsafe { dlsym(handle, name.as_ptr()) };
        if pointer.is_null() {
            Err(SpaceError::ApiUnavailable(format!(
                "missing SkyLight symbol {}: {}",
                name.to_string_lossy(),
                unsafe { last_dl_error() }
            )))
        } else {
            Ok(pointer)
        }
    }

    unsafe fn symbol_any(handle: *mut c_void, names: &[&CStr]) -> Result<*mut c_void, SpaceError> {
        for name in names {
            if let Ok(pointer) = unsafe { symbol(handle, name) } {
                return Ok(pointer);
            }
        }
        Err(SpaceError::ApiUnavailable(format!(
            "missing SkyLight symbols {}",
            names
                .iter()
                .map(|name| name.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" or ")
        )))
    }

    unsafe fn last_dl_error() -> String {
        let error = unsafe { dlerror() };
        if error.is_null() {
            "unknown dynamic loader error".to_string()
        } else {
            unsafe { CStr::from_ptr(error) }
                .to_string_lossy()
                .into_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_ordinals_ignore_fullscreen_and_system_spaces() {
        let desktops = macos::normalize_desktops([(42, 0), (51, 4), (63, 2), (87, 0)]);

        assert_eq!(
            desktops,
            [
                DesktopSpace { id: 42, ordinal: 1 },
                DesktopSpace { id: 87, ordinal: 2 },
            ]
        );
    }

    #[test]
    fn resolves_a_persisted_display_and_desktop_ordinal() {
        let inventory = SpaceInventory {
            displays: vec![DisplaySpaces {
                uuid: "Main".to_string(),
                current_space_id: 42,
                desktops: vec![
                    DesktopSpace { id: 42, ordinal: 1 },
                    DesktopSpace { id: 87, ordinal: 2 },
                ],
            }],
        };

        let desktop = macos::resolve_desktop(
            &inventory,
            &DesktopPlacement {
                display_uuid: "Main".to_string(),
                desktop_ordinal: 2,
            },
        )
        .unwrap();

        assert_eq!(desktop.id, 87);
    }

    #[test]
    fn reports_a_missing_display_separately_from_a_missing_desktop() {
        let inventory = SpaceInventory {
            displays: vec![DisplaySpaces {
                uuid: "Main".to_string(),
                current_space_id: 42,
                desktops: vec![DesktopSpace { id: 42, ordinal: 1 }],
            }],
        };

        assert!(matches!(
            macos::resolve_desktop(
                &inventory,
                &DesktopPlacement {
                    display_uuid: "External".to_string(),
                    desktop_ordinal: 1,
                }
            ),
            Err(SpaceError::DisplayUnavailable { .. })
        ));
        assert!(matches!(
            macos::resolve_desktop(
                &inventory,
                &DesktopPlacement {
                    display_uuid: "Main".to_string(),
                    desktop_ordinal: 3,
                }
            ),
            Err(SpaceError::DesktopUnavailable { .. })
        ));
    }

    #[test]
    fn creates_only_the_minimum_number_of_missing_desktops() {
        assert_eq!(macos::missing_desktop_count(1, 3), 2);
        assert_eq!(macos::missing_desktop_count(3, 3), 0);
        assert_eq!(macos::missing_desktop_count(4, 3), 0);
    }

    #[test]
    fn uses_workspace_compatibility_move_on_sonoma_14_5_and_newer() {
        assert!(!macos::version_requires_compat_move("14.4.1"));
        assert!(macos::version_requires_compat_move("14.5"));
        assert!(macos::version_requires_compat_move("15.0"));
        assert!(macos::version_requires_compat_move("26.0"));
    }
}
