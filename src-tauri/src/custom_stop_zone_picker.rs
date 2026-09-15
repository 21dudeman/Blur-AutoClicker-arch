use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{GetLastError, LPARAM, LRESULT, WPARAM};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK,
    KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_MOUSEMOVE,
    WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
};

#[cfg(target_os = "linux")]
use crate::x11::vkcodes::*;

use crate::engine::mouse::{current_virtual_screen_rect, VirtualScreenRect};
use crate::error::poisoned_inner;
use crate::error::AppError;
use crate::error::AppResult;
use crate::ClickerState;

const PREVIEW_EMIT_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StopZoneRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MouseHookDecision {
    StartDrawing,
    FinishDrawing,
    Pass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeyboardHookDecision {
    Pass,
    Cancel,
}

#[derive(Default)]
struct PickerRuntime {
    active: bool,
    drawing_start: Option<(i32, i32)>,
    #[cfg(target_os = "windows")]
    mouse_hook: HHOOK,
    #[cfg(target_os = "windows")]
    keyboard_hook: HHOOK,
    #[cfg(target_os = "windows")]
    thread_id: u32,
    app: Option<AppHandle>,
    last_preview_emit: Option<Instant>,
}

unsafe impl Send for PickerRuntime {}
unsafe impl Sync for PickerRuntime {}

static PICKER: OnceLock<Mutex<PickerRuntime>> = OnceLock::new();

fn picker() -> &'static Mutex<PickerRuntime> {
    PICKER.get_or_init(|| Mutex::new(PickerRuntime::default()))
}

fn classify_mouse_message(message: u32, drawing: bool) -> MouseHookDecision {
    match message {
        WM_RBUTTONDOWN => MouseHookDecision::StartDrawing,
        WM_RBUTTONUP if drawing => MouseHookDecision::FinishDrawing,
        _ => MouseHookDecision::Pass,
    }
}

fn should_emit_drag_preview(drawing: bool) -> bool {
    drawing
}

fn classify_keyboard_message(message: u32, virtual_key: u32) -> KeyboardHookDecision {
    match (message, virtual_key) {
        (WM_KEYDOWN | WM_SYSKEYDOWN, key) if key == VK_ESCAPE as u32 => {
            KeyboardHookDecision::Cancel
        }
        _ => KeyboardHookDecision::Pass,
    }
}

fn normalize_rect(start: (i32, i32), end: (i32, i32)) -> StopZoneRect {
    let left = start.0.min(end.0);
    let top = start.1.min(end.1);
    let right = start.0.max(end.0);
    let bottom = start.1.max(end.1);

    StopZoneRect {
        x: left,
        y: top,
        width: right - left + 1,
        height: bottom - top + 1,
    }
}

// ---------------------------------------------------------------------------
// Windows hook-based listener
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
pub fn start_custom_stop_zone_pick_inner(app: AppHandle) -> AppResult<()> {
    crate::click_point_picker::cancel_click_point_pick_inner(&app);

    {
        let mut runtime = picker().lock().unwrap_or_else(poisoned_inner);
        if runtime.active {
            crate::overlay::show_custom_stop_zone_pick_overlay(&app)?;
            return Ok(());
        }

        runtime.active = true;
        runtime.drawing_start = None;
        runtime.app = Some(app.clone());
        runtime.last_preview_emit = None;
    }

    app.state::<ClickerState>()
        .custom_stop_zone_pick_active
        .store(true, std::sync::atomic::Ordering::SeqCst);

    crate::overlay::show_custom_stop_zone_pick_overlay(&app)?;

    let (ready_tx, ready_rx) = mpsc::channel();
    std::thread::spawn(move || unsafe {
        let thread_id = GetCurrentThreadId();
        let mouse_hook =
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), std::ptr::null_mut(), 0);
        if mouse_hook.is_null() {
            let err = GetLastError();
            let _ = ready_tx.send(Err(AppError::WindowsSystem(err)));
            return;
        }

        let keyboard_hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_hook_proc),
            std::ptr::null_mut(),
            0,
        );
        if keyboard_hook.is_null() {
            let err = GetLastError();
            UnhookWindowsHookEx(mouse_hook);
            let _ = ready_tx.send(Err(AppError::WindowsSystem(err)));
            return;
        }

        {
            let mut runtime = picker().lock().unwrap_or_else(poisoned_inner);
            runtime.mouse_hook = mouse_hook;
            runtime.keyboard_hook = keyboard_hook;
            runtime.thread_id = thread_id;
        }
        let _ = ready_tx.send(Ok(()));

        let mut msg = std::mem::zeroed::<MSG>();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}

        UnhookWindowsHookEx(mouse_hook);
        UnhookWindowsHookEx(keyboard_hook);
        let mut runtime = picker().lock().unwrap_or_else(poisoned_inner);
        if runtime.mouse_hook == mouse_hook {
            runtime.mouse_hook = std::ptr::null_mut();
        }
        if runtime.keyboard_hook == keyboard_hook {
            runtime.keyboard_hook = std::ptr::null_mut();
        }
        if runtime.mouse_hook.is_null() && runtime.keyboard_hook.is_null() {
            runtime.thread_id = 0;
        }
    });

    match ready_rx.recv_timeout(Duration::from_secs(1)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            cancel_custom_stop_zone_pick_inner(&app);
            Err(error)
        }
        Err(_) => {
            cancel_custom_stop_zone_pick_inner(&app);
            Err(AppError::ChannelFailure)
        }
    }
}

// ---------------------------------------------------------------------------
// Linux X11 pointer-grab + polling listener
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
pub fn start_custom_stop_zone_pick_inner(app: AppHandle) -> AppResult<()> {
    crate::click_point_picker::cancel_click_point_pick_inner(&app);

    {
        let mut runtime = picker().lock().unwrap_or_else(poisoned_inner);
        if runtime.active {
            crate::overlay::show_custom_stop_zone_pick_overlay(&app)?;
            return Ok(());
        }

        runtime.active = true;
        runtime.drawing_start = None;
        runtime.app = Some(app.clone());
        runtime.last_preview_emit = None;
    }

    app.state::<ClickerState>()
        .custom_stop_zone_pick_active
        .store(true, std::sync::atomic::Ordering::SeqCst);

    crate::overlay::show_custom_stop_zone_pick_overlay(&app)?;

    let (ready_tx, ready_rx) = mpsc::channel();

    std::thread::spawn(move || {
        if !crate::x11::begin_pointer_redirect() {
            let _ = ready_tx.send(Err(AppError::ChannelFailure));
            return;
        }
        let _ = ready_tx.send(Ok(()));

        let mut right_was_pressed = false;

        loop {
            {
                let runtime = picker().lock().unwrap_or_else(poisoned_inner);
                if !runtime.active {
                    break;
                }
            }

            std::thread::sleep(Duration::from_millis(8));

            let mask = crate::x11::pointer_button_mask();
            let right = (mask & crate::x11::Button3Mask) != 0;
            let pos = crate::x11::cursor_position().unwrap_or((0, 0));

            let drawing = picker()
                .lock()
                .unwrap_or_else(poisoned_inner)
                .drawing_start
                .is_some();

            if drawing {
                if let Some(start) = picker().lock().unwrap_or_else(poisoned_inner).drawing_start {
                    emit_preview(start, (pos.0, pos.1), false);
                }
            } else {
                emit_cursor_position(pos.0, pos.1);
            }

            if right && !right_was_pressed {
                let mut runtime = picker().lock().unwrap_or_else(poisoned_inner);
                runtime.drawing_start = Some((pos.0, pos.1));
                runtime.last_preview_emit = None;
                drop(runtime);
                emit_preview((pos.0, pos.1), (pos.0, pos.1), true);
            } else if !right && right_was_pressed {
                let start = picker().lock().unwrap_or_else(poisoned_inner).drawing_start;
                if let Some(start) = start {
                    finish_custom_stop_zone_pick(normalize_rect(start, (pos.0, pos.1)));
                    break;
                }
            }

            if crate::x11::is_key_name_pressed("Escape") {
                cancel_custom_stop_zone_pick_from_hook();
                break;
            }

            right_was_pressed = right;
        }

        crate::x11::end_pointer_redirect();
    });

    match ready_rx.recv_timeout(Duration::from_secs(1)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            cancel_custom_stop_zone_pick_inner(&app);
            Err(error)
        }
        Err(_) => {
            cancel_custom_stop_zone_pick_inner(&app);
            Err(AppError::ChannelFailure)
        }
    }
}

// ---------------------------------------------------------------------------
// Shared cleanup / cancellation
// ---------------------------------------------------------------------------

pub fn cancel_custom_stop_zone_pick_inner(app: &AppHandle) {
    stop_custom_stop_zone_pick(Some(app.clone()), true);
    let _ = crate::overlay::hide_custom_stop_zone_pick_overlay(app);
}

fn cancel_custom_stop_zone_pick_from_hook() {
    let app = stop_custom_stop_zone_pick(None, true);
    if let Some(app) = app {
        let _ = crate::overlay::hide_custom_stop_zone_pick_overlay(&app);
    }
}

fn finish_custom_stop_zone_pick(rect: StopZoneRect) {
    let app: Option<AppHandle> = {
        let runtime = picker().lock().unwrap_or_else(poisoned_inner);
        if !runtime.active {
            return;
        }
        runtime.app.clone()
    };
    if let Some(ref app) = app {
        let _ = app.emit("custom-stop-zone-picked", rect);
    }
    stop_custom_stop_zone_pick(None, true);
}

fn stop_custom_stop_zone_pick(
    app_override: Option<AppHandle>,
    notify_overlay: bool,
) -> Option<AppHandle> {
    let (app, _thread_id) = {
        let mut runtime = picker().lock().unwrap_or_else(poisoned_inner);
        let app = app_override.or_else(|| runtime.app.clone());
        #[cfg(target_os = "windows")]
        let _thread_id = runtime.thread_id;
        #[cfg(not(target_os = "windows"))]
        let _thread_id = 0u32;
        runtime.active = false;
        runtime.drawing_start = None;
        runtime.app = None;
        runtime.last_preview_emit = None;
        (app, _thread_id)
    };

    if let Some(app) = &app {
        app.state::<ClickerState>()
            .custom_stop_zone_pick_active
            .store(false, std::sync::atomic::Ordering::SeqCst);
        let _ = app.emit("custom-stop-zone-pick-ended", ());
        if notify_overlay {
            let _ = crate::overlay::set_custom_stop_zone_pick_mode(app, false);
        }
    }

    #[cfg(target_os = "windows")]
    if _thread_id != 0 {
        unsafe {
            PostThreadMessageW(_thread_id, WM_QUIT, 0, 0);
        }
    }

    app
}

// ---------------------------------------------------------------------------
// Windows hook procs
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam);
    }

    let message = wparam as u32;
    let mouse = &*(lparam as *const MSLLHOOKSTRUCT);
    let drawing = picker()
        .lock()
        .unwrap_or_else(poisoned_inner)
        .drawing_start
        .is_some();

    if message == WM_MOUSEMOVE {
        if should_emit_drag_preview(drawing) {
            let start = picker().lock().unwrap_or_else(poisoned_inner).drawing_start;
            if let Some(start) = start {
                emit_preview(start, (mouse.pt.x, mouse.pt.y), false);
            }
        } else {
            emit_cursor_position(mouse.pt.x, mouse.pt.y);
        }
        return CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam);
    }

    match classify_mouse_message(message, drawing) {
        MouseHookDecision::Pass => CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam),
        MouseHookDecision::StartDrawing => {
            {
                let mut runtime = picker().lock().unwrap_or_else(poisoned_inner);
                runtime.drawing_start = Some((mouse.pt.x, mouse.pt.y));
                runtime.last_preview_emit = None;
            }
            emit_preview((mouse.pt.x, mouse.pt.y), (mouse.pt.x, mouse.pt.y), true);
            1
        }
        MouseHookDecision::FinishDrawing => {
            let start = picker().lock().unwrap_or_else(poisoned_inner).drawing_start;
            if let Some(start) = start {
                finish_custom_stop_zone_pick(normalize_rect(start, (mouse.pt.x, mouse.pt.y)));
            }
            1
        }
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam);
    }

    let message = wparam as u32;
    let keyboard = &*(lparam as *const KBDLLHOOKSTRUCT);

    match classify_keyboard_message(message, keyboard.vkCode) {
        KeyboardHookDecision::Pass => CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam),
        KeyboardHookDecision::Cancel => {
            cancel_custom_stop_zone_pick_from_hook();
            1
        }
    }
}

// ---------------------------------------------------------------------------
// Emitter helpers (shared across platforms)
// ---------------------------------------------------------------------------

fn emit_cursor_position(x: i32, y: i32) {
    let app = {
        let mut runtime = picker().lock().unwrap_or_else(poisoned_inner);
        if !runtime.active {
            return;
        }

        let now = Instant::now();
        if runtime
            .last_preview_emit
            .is_some_and(|last| now.duration_since(last) < PREVIEW_EMIT_INTERVAL)
        {
            return;
        }
        runtime.last_preview_emit = Some(now);
        runtime.app.clone()
    };

    if let Some(app) = app {
        let Some(bounds) = current_virtual_screen_rect() else {
            return;
        };
        let cursor = VirtualScreenRect::new(x, y, 1, 1).offset_from(bounds);

        let _ = app.emit(
            "custom-stop-zone-preview",
            serde_json::json!({
                "cursorX": cursor.left,
                "cursorY": cursor.top,
            }),
        );
    }
}

fn emit_preview(start: (i32, i32), end: (i32, i32), force: bool) {
    let app = {
        let mut runtime = picker().lock().unwrap_or_else(poisoned_inner);
        if !runtime.active {
            return;
        }

        let now = Instant::now();
        if !force
            && runtime
                .last_preview_emit
                .is_some_and(|last| now.duration_since(last) < PREVIEW_EMIT_INTERVAL)
        {
            return;
        }
        runtime.last_preview_emit = Some(now);
        runtime.app.clone()
    };

    if let Some(app) = app {
        let rect = normalize_rect(start, end);
        let Some(bounds) = current_virtual_screen_rect() else {
            return;
        };
        let offset =
            VirtualScreenRect::new(rect.x, rect.y, rect.width, rect.height).offset_from(bounds);
        let cursor = VirtualScreenRect::new(end.0, end.1, 1, 1).offset_from(bounds);

        let _ = app.emit(
            "custom-stop-zone-preview",
            serde_json::json!({
                "x": offset.left,
                "y": offset.top,
                "width": offset.width,
                "height": offset.height,
                "cursorX": cursor.left,
                "cursorY": cursor.top,
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_keyboard_message, classify_mouse_message, normalize_rect,
        should_emit_drag_preview, KeyboardHookDecision, MouseHookDecision, StopZoneRect,
    };
    #[cfg(target_os = "linux")]
    use crate::x11::vkcodes::{
        VK_ESCAPE, VK_SPACE, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_RBUTTONDOWN,
        WM_RBUTTONUP, WM_SYSKEYDOWN,
    };
    #[cfg(target_os = "windows")]
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_SPACE};
    #[cfg(target_os = "windows")]
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_RBUTTONDOWN, WM_RBUTTONUP,
        WM_SYSKEYDOWN,
    };

    #[test]
    fn right_button_down_starts_drawing() {
        assert_eq!(
            classify_mouse_message(WM_RBUTTONDOWN, false),
            MouseHookDecision::StartDrawing
        );
    }

    #[test]
    fn mouse_move_updates_preview_only_while_drawing() {
        assert_eq!(
            classify_mouse_message(WM_MOUSEMOVE, false),
            MouseHookDecision::Pass
        );
        assert_eq!(
            classify_mouse_message(WM_MOUSEMOVE, true),
            MouseHookDecision::Pass
        );
        assert!(should_emit_drag_preview(true));
        assert!(!should_emit_drag_preview(false));
    }

    #[test]
    fn right_button_up_finishes_only_while_drawing() {
        assert_eq!(
            classify_mouse_message(WM_RBUTTONUP, true),
            MouseHookDecision::FinishDrawing
        );
        assert_eq!(
            classify_mouse_message(WM_RBUTTONUP, false),
            MouseHookDecision::Pass
        );
    }

    #[test]
    fn left_button_events_pass_through() {
        assert_eq!(
            classify_mouse_message(WM_LBUTTONDOWN, false),
            MouseHookDecision::Pass
        );
    }

    #[test]
    fn escape_key_down_cancels_picker() {
        assert_eq!(
            classify_keyboard_message(WM_KEYDOWN, VK_ESCAPE as u32),
            KeyboardHookDecision::Cancel
        );
        assert_eq!(
            classify_keyboard_message(WM_SYSKEYDOWN, VK_ESCAPE as u32),
            KeyboardHookDecision::Cancel
        );
    }

    #[test]
    fn other_key_events_pass_through() {
        assert_eq!(
            classify_keyboard_message(WM_KEYUP, VK_ESCAPE as u32),
            KeyboardHookDecision::Pass
        );
        assert_eq!(
            classify_keyboard_message(WM_KEYDOWN, VK_SPACE as u32),
            KeyboardHookDecision::Pass
        );
    }

    #[test]
    fn reverse_drag_normalizes_to_positive_size() {
        assert_eq!(
            normalize_rect((200, 150), (100, 75)),
            StopZoneRect {
                x: 100,
                y: 75,
                width: 101,
                height: 76,
            }
        );
    }
}
