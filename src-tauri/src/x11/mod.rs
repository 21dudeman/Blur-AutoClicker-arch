// Cross-platform replacement for the Win32 input/process/hotkey layer.
// On Linux (X11 + XTEST) we drive the same VK-code abstraction the Windows
// code uses, so the higher-level engine/hotkey logic stays platform-agnostic.
//
// Everything is implemented through hand-written FFI (libX11, libXtst,
// libXinerama); no extra crates are required beyond the link lines emitted
// from build.rs.
#![allow(non_camel_case_types)]
#![allow(dead_code)]

pub mod vkcodes;

use std::ffi::c_char;
use std::ffi::c_int;
use std::ffi::c_long;
use std::ffi::c_uchar;
use std::ffi::c_uint;
use std::ffi::c_ulong;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use vkcodes::*;

// -- opaque X11 handles -----------------------------------------------------

#[repr(C)]
pub enum Display {
    __hidden,
}

pub type Window = c_ulong;
pub type Atom = c_ulong;
pub type KeyCode = c_uint;
pub type KeySym = c_ulong;

// -- Xlib declarations ------------------------------------------------------

extern "C" {
    fn XOpenDisplay(name: *const c_char) -> *mut Display;
    fn XCloseDisplay(display: *mut Display) -> c_int;
    fn XInitThreads() -> c_int;
    fn XDefaultScreen(display: *mut Display) -> c_int;
    fn XDefaultRootWindow(display: *mut Display) -> Window;
    fn XRootWindow(display: *mut Display, screen: c_int) -> Window;
    fn XDisplayWidth(display: *mut Display, screen: c_int) -> c_int;
    fn XDisplayHeight(display: *mut Display, screen: c_int) -> c_int;
    fn XFlush(display: *mut Display) -> c_int;
    fn XSync(display: *mut Display, discard: c_int) -> c_int;
    fn XQueryPointer(
        display: *mut Display,
        window: Window,
        root_return: *mut Window,
        child_return: *mut Window,
        root_x_return: *mut c_int,
        root_y_return: *mut c_int,
        win_x_return: *mut c_int,
        win_y_return: *mut c_int,
        mask_return: *mut c_uint,
    ) -> c_int;
    fn XQueryKeymap(display: *mut Display, keys_return: *mut c_char) -> c_int;
    fn XKeysymToKeycode(display: *mut Display, keysym: c_ulong) -> KeyCode;
    fn XStringToKeysym(name: *const c_char) -> KeySym;
    fn XInternAtom(display: *mut Display, name: *const c_char, only_if_exists: c_int) -> Atom;
    fn XGetWindowProperty(
        display: *mut Display,
        w: Window,
        property: Atom,
        long_offset: c_long,
        long_length: c_long,
        delete: c_int,
        req_type: Atom,
        actual_type_return: *mut Atom,
        actual_format_return: *mut c_int,
        nitems_return: *mut c_ulong,
        bytes_after_return: *mut c_ulong,
        prop_return: *mut *mut c_uchar,
    ) -> c_int;
    fn XFree(data: *mut c_void) -> c_int;
    fn XFetchName(display: *mut Display, w: Window, window_name_return: *mut *mut c_char) -> c_int;
    fn XKeycodeToKeysym(display: *mut Display, keycode: KeyCode, index: c_int) -> KeySym;
    fn XGetKeyboardControl(display: *mut Display, keyboard_state: *mut XKeyboardState) -> c_int;
    fn XGetInputFocus(display: *mut Display, focus_return: *mut Window, revert_to_return: *mut c_int)
        -> c_int;
    fn XQueryTree(
        display: *mut Display,
        w: Window,
        root_return: *mut Window,
        parent_return: *mut Window,
        children_return: *mut *mut Window,
        nchildren_return: *mut c_uint,
    ) -> c_int;
    fn XGrabPointer(
        display: *mut Display,
        grab_window: Window,
        owner_events: c_int,
        event_mask: c_uint,
        pointer_mode: c_int,
        keyboard_mode: c_int,
        confine_to: Window,
        cursor: Window,
        time: c_ulong,
    ) -> c_int;
    fn XUngrabPointer(display: *mut Display, time: c_ulong) -> c_int;
}

// -- libXtst (XTEST extension) ----------------------------------------------

extern "C" {
    fn XTestFakeMotionEvent(
        display: *mut Display,
        screen_number: c_int,
        x: c_int,
        y: c_int,
        delay: c_ulong,
    ) -> c_int;
    fn XTestFakeButtonEvent(
        display: *mut Display,
        button: c_uint,
        is_press: c_int,
        delay: c_ulong,
    ) -> c_int;
    fn XTestFakeKeyEvent(
        display: *mut Display,
        keycode: c_uint,
        is_press: c_int,
        delay: c_ulong,
    ) -> c_int;
}

// -- Xinerama ---------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct XineramaScreenInfo {
    pub screen_number: c_int,
    pub x_org: c_int,
    pub y_org: c_int,
    pub width: c_int,
    pub height: c_int,
}

extern "C" {
    fn XineramaQueryExtension(
        display: *mut Display,
        event_base_return: *mut c_int,
        error_base_return: *mut c_int,
    ) -> c_int;
    fn XineramaIsActive(display: *mut Display) -> c_int;
    fn XineramaQueryScreens(
        display: *mut Display,
        number_return: *mut c_int,
    ) -> *mut XineramaScreenInfo;
}

// -- secondary structs ------------------------------------------------------

#[repr(C)]
pub struct XKeyboardState {
    pub key_click_percent: c_int,
    pub bell_percent: c_int,
    pub bell_pitch: c_uint,
    pub bell_duration: c_uint,
    pub led_mask: c_int,
    pub global_auto_repeat: c_int,
    pub auto_repeats: [c_char; 32],
}

// -- event masks (X.h) ------------------------------------------------------

pub const Button1Mask: c_uint = 0x100; // left
pub const Button2Mask: c_uint = 0x200; // middle
pub const Button3Mask: c_uint = 0x400; // right
const ButtonPressMask: c_uint = 0x0000_0004;
const ButtonReleaseMask: c_uint = 0x0000_0008;
const PointerMotionMask: c_uint = 0x0000_0002;
const GrabModeAsync: c_int = 1;

// -- keysyms (keysymdef.h) --------------------------------------------------

const XK_space: KeySym = 0x0020;
const XK_BackSpace: KeySym = 0xff08;
const XK_Tab: KeySym = 0xff09;
const XK_Return: KeySym = 0xff0d;
const XK_Escape: KeySym = 0xff1b;
const XK_Pause: KeySym = 0xff13;
const XK_Scroll_Lock: KeySym = 0xff14;
const XK_Caps_Lock: KeySym = 0xffe5;
const XK_Num_Lock: KeySym = 0xff7f;
const XK_Home: KeySym = 0xff50;
const XK_Left: KeySym = 0xff51;
const XK_Up: KeySym = 0xff52;
const XK_Right: KeySym = 0xff53;
const XK_Down: KeySym = 0xff54;
const XK_Prior: KeySym = 0xff55;
const XK_Next: KeySym = 0xff56;
const XK_End: KeySym = 0xff57;
const XK_Insert: KeySym = 0xff63;
const XK_Delete: KeySym = 0xffff;
const XK_Print: KeySym = 0xff61;
const XK_Menu: KeySym = 0xff67;
const XK_Help: KeySym = 0xff6a;
const XK_KP_Enter: KeySym = 0xff8d;
const XK_KP_Multiply: KeySym = 0xffaa;
const XK_KP_Add: KeySym = 0xffab;
const XK_KP_Subtract: KeySym = 0xffad;
const XK_KP_Decimal: KeySym = 0xffae;
const XK_KP_Divide: KeySym = 0xffaf;
const XK_KP_0: KeySym = 0xffb0;
const XK_KP_9: KeySym = 0xffb9;
const XK_F1: KeySym = 0xffbe;
const XK_F24: KeySym = 0xffd5;
const XK_Shift_L: KeySym = 0xffe1;
const XK_Shift_R: KeySym = 0xffe2;
const XK_Control_L: KeySym = 0xffe3;
const XK_Control_R: KeySym = 0xffe4;
const XK_Meta_L: KeySym = 0xffe7;
const XK_Meta_R: KeySym = 0xffe8;
const XK_Alt_L: KeySym = 0xffe9;
const XK_Alt_R: KeySym = 0xffea;
const XK_Super_L: KeySym = 0xffeb;
const XK_Super_R: KeySym = 0xffec;

const XK_less: KeySym = 0x3c;

// -- per-thread display -----------------------------------------------------

thread_local! {
    static DISPLAY: Mutex<*mut Display> = Mutex::new(std::ptr::null_mut());
}

// Poison-tolerant per-thread display accessor (fresh connection per thread,
// since Xlib sockets must not be shared across threads).
fn display_ptr() -> *mut Display {
    // Xlib must be initialised for multithreaded use before any call, or the
    // internal global state races across the GUI thread and our poll/inject
    // threads. XInitThreads is idempotent but we run it exactly once.
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        unsafe {
            XInitThreads();
        }
    });
    DISPLAY.with(|slot| {
        let mut g = match slot.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        if g.is_null() {
            // One connection per thread: Xlib sockets are not shared.
            let d = unsafe { XOpenDisplay(std::ptr::null()) };
            *g = d;
        }
        *g
    })
}

fn close_thread_display() {
    DISPLAY.with(|slot| {
        let mut g = match slot.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        if !g.is_null() {
            unsafe {
                XCloseDisplay(*g);
            }
            *g = std::ptr::null_mut();
        }
    });
}

fn is_connected() -> bool {
    !display_ptr().is_null()
}

// -- modifiers / keysym mapping ----------------------------------------------

fn vk_to_keysym(vk: u16) -> KeySym {
    match vk {
        0x30..=0x39 => vk as KeySym,
        VK_A..=VK_Z => (vk | 0x20) as KeySym, // lowercase registration of the physical key
        VK_SPACE => XK_space,
        VK_BACK => XK_BackSpace,
        VK_TAB => XK_Tab,
        VK_RETURN => XK_Return,
        VK_ESCAPE => XK_Escape,
        VK_PAUSE => XK_Pause,
        VK_SCROLL => XK_Scroll_Lock,
        VK_CAPITAL => XK_Caps_Lock,
        VK_NUMLOCK => XK_Num_Lock,
        VK_HOME => XK_Home,
        VK_END => XK_End,
        VK_LEFT => XK_Left,
        VK_UP => XK_Up,
        VK_RIGHT => XK_Right,
        VK_DOWN => XK_Down,
        VK_PRIOR => XK_Prior,
        VK_NEXT => XK_Next,
        VK_INSERT => XK_Insert,
        VK_DELETE => XK_Delete,
        VK_SNAPSHOT => XK_Print,
        VK_APPS => XK_Menu,
        VK_HELP => XK_Help,
        VK_NUMPAD0..=VK_NUMPAD9 => XK_KP_0 + (vk - VK_NUMPAD0) as KeySym,
        VK_MULTIPLY => XK_KP_Multiply,
        VK_ADD => XK_KP_Add,
        VK_SUBTRACT => XK_KP_Subtract,
        VK_DECIMAL => XK_KP_Decimal,
        VK_DIVIDE => XK_KP_Divide,
        VK_F1..=VK_F24 => XK_F1 + (vk - VK_F1) as KeySym,
        VK_SHIFT | VK_LSHIFT => XK_Shift_L,
        VK_RSHIFT => XK_Shift_R,
        VK_CONTROL | VK_LCONTROL => XK_Control_L,
        VK_RCONTROL => XK_Control_R,
        VK_MENU | VK_LMENU => XK_Alt_L,
        VK_RMENU => XK_Alt_R,
        VK_LWIN => XK_Super_L,
        VK_RWIN => XK_Super_R,
        VK_OEM_1 => 0x3b, // ;:
        VK_OEM_PLUS => 0x3d, // =+
        VK_OEM_COMMA => 0x2c, // ,<
        VK_OEM_MINUS => 0x2d, // -_
        VK_OEM_PERIOD => 0x2e, // .>
        VK_OEM_2 => 0x2f, // /?
        VK_OEM_3 => 0x60, // `~
        VK_OEM_4 => 0x5b, // [{
        VK_OEM_5 => 0x5c, // \|
        VK_OEM_6 => 0x5d, // ]}
        VK_OEM_7 => 0x27, // '"
        VK_OEM_8 | VK_OEM_102 => XK_less, // IntlBackslash
        _ => 0,
    }
}

fn keysym_to_vk(keysym: KeySym) -> u16 {
    if (0x41..=0x5a).contains(&keysym) {
        return keysym as u16;
    }
    if (0x61..=0x7a).contains(&keysym) {
        return (keysym as u16) - 0x20;
    }
    if (0x30..=0x39).contains(&keysym) {
        return keysym as u16;
    }
    if (XK_F1..=XK_F24).contains(&keysym) {
        return VK_F1 + (keysym - XK_F1) as u16;
    }
    if (XK_KP_0..=XK_KP_9).contains(&keysym) {
        return VK_NUMPAD0 + (keysym - XK_KP_0) as u16;
    }
    match keysym {
        XK_space => VK_SPACE,
        XK_BackSpace => VK_BACK,
        XK_Tab => VK_TAB,
        XK_Return => VK_RETURN,
        XK_Escape => VK_ESCAPE,
        XK_Pause => VK_PAUSE,
        XK_Scroll_Lock => VK_SCROLL,
        XK_Caps_Lock => VK_CAPITAL,
        XK_Num_Lock => VK_NUMLOCK,
        XK_Home => VK_HOME,
        XK_End => VK_END,
        XK_Left => VK_LEFT,
        XK_Up => VK_UP,
        XK_Right => VK_RIGHT,
        XK_Down => VK_DOWN,
        XK_Prior => VK_PRIOR,
        XK_Next => VK_NEXT,
        XK_Insert => VK_INSERT,
        XK_Delete => VK_DELETE,
        XK_Print => VK_SNAPSHOT,
        XK_Menu => VK_APPS,
        XK_KP_Enter => VK_RETURN,
        XK_KP_Multiply => VK_MULTIPLY,
        XK_KP_Add => VK_ADD,
        XK_KP_Subtract => VK_SUBTRACT,
        XK_KP_Decimal => VK_DECIMAL,
        XK_KP_Divide => VK_DIVIDE,
        XK_Shift_L => VK_LSHIFT,
        XK_Shift_R => VK_RSHIFT,
        XK_Control_L => VK_LCONTROL,
        XK_Control_R => VK_RCONTROL,
        XK_Alt_L => VK_LMENU,
        XK_Alt_R => VK_RMENU,
        XK_Super_L | XK_Meta_L => VK_LWIN,
        XK_Super_R | XK_Meta_R => VK_RWIN,
        0x3b => VK_OEM_1,
        0x3d => VK_OEM_PLUS,
        0x2c => VK_OEM_COMMA,
        0x2d => VK_OEM_MINUS,
        0x2e => VK_OEM_PERIOD,
        0x2f => VK_OEM_2,
        0x60 => VK_OEM_3,
        0x5b => VK_OEM_4,
        0x5c => VK_OEM_5,
        0x5d => VK_OEM_6,
        0x27 => VK_OEM_7,
        XK_less => VK_OEM_8,
        _ => 0,
    }
}

// -- mouse position / screen -------------------------------------------------

pub fn cursor_position() -> Option<(i32, i32)> {
    let d = display_ptr();
    if d.is_null() {
        return None;
    }
    let root = unsafe { XDefaultRootWindow(d) };
    let mut rx: c_int = 0;
    let mut ry: c_int = 0;
    let mut wx: c_int = 0;
    let mut wy: c_int = 0;
    let mut mask: c_uint = 0;
    let mut rw: Window = 0;
    let mut cw: Window = 0;
    let ok = unsafe {
        XQueryPointer(
            d,
            root,
            &mut rw,
            &mut cw,
            &mut rx,
            &mut ry,
            &mut wx,
            &mut wy,
            &mut mask,
        )
    };
    if ok == 0 {
        None
    } else {
        Some((rx, ry))
    }
}

pub fn pointer_button_mask() -> c_uint {
    let d = display_ptr();
    if d.is_null() {
        return 0;
    }
    let root = unsafe { XDefaultRootWindow(d) };
    let mut rw: Window = 0;
    let mut cw: Window = 0;
    let mut rx: c_int = 0;
    let mut ry: c_int = 0;
    let mut wx: c_int = 0;
    let mut wy: c_int = 0;
    let mut mask: c_uint = 0;
    unsafe {
        XQueryPointer(
            d,
            root,
            &mut rw,
            &mut cw,
            &mut rx,
            &mut ry,
            &mut wx,
            &mut wy,
            &mut mask,
        )
    };
    mask
}

pub fn virtual_screen_rect() -> Option<(i32, i32, usize, usize)> {
    let d = display_ptr();
    if d.is_null() {
        return None;
    }
    let screen = unsafe { XDefaultScreen(d) };
    let w = unsafe { XDisplayWidth(d, screen) };
    let h = unsafe { XDisplayHeight(d, screen) };
    if w <= 0 || h <= 0 {
        return None;
    }
    Some((0, 0, w as usize, h as usize))
}

pub fn monitor_rects() -> Option<Vec<(i32, i32, usize, usize)>> {
    let d = display_ptr();
    if d.is_null() {
        return None;
    }
    let mut rects = Vec::new();
    let mut event_base: c_int = 0;
    let mut error_base: c_int = 0;
    let available = unsafe { XineramaQueryExtension(d, &mut event_base, &mut error_base) };
    if available != 0 && unsafe { XineramaIsActive(d) } != 0 {
        let mut count: c_int = 0;
        let infos = unsafe { XineramaQueryScreens(d, &mut count) };
        if !infos.is_null() && count > 0 {
            for i in 0..count as isize {
                let info = unsafe { &*infos.offset(i) };
                if info.width > 0 && info.height > 0 {
                    rects.push((info.x_org, info.y_org, info.width as usize, info.height as usize));
                }
            }
            unsafe { XFree(infos as *mut c_void) };
        }
    }
    if rects.is_empty() {
        rects.push(virtual_screen_rect()?);
    }
    rects.sort_by_key(|r| (r.1, r.0));
    Some(rects)
}

// -- synthetic input ---------------------------------------------------------

pub fn move_pointer(x: i32, y: i32) {
    let d = display_ptr();
    if d.is_null() {
        return;
    }
    let screen = unsafe { XDefaultScreen(d) };
    unsafe {
        XTestFakeMotionEvent(d, screen, x, y, 0);
        XFlush(d);
    }
}

pub fn press_button(button: u32) {
    let d = display_ptr();
    if d.is_null() {
        return;
    }
    unsafe {
        XTestFakeButtonEvent(d, button, 1, 0);
        XFlush(d);
    }
}

pub fn release_button(button: u32) {
    let d = display_ptr();
    if d.is_null() {
        return;
    }
    unsafe {
        XTestFakeButtonEvent(d, button, 0, 0);
        XFlush(d);
    }
}

pub fn vk_keycode(vk: u16) -> Option<KeyCode> {
    let d = display_ptr();
    if d.is_null() {
        return None;
    }
    let keysym = vk_to_keysym(vk);
    if keysym == 0 {
        return None;
    }
    let keycode = unsafe { XKeysymToKeycode(d, keysym) };
    if keycode == 0 {
        None
    } else {
        Some(keycode)
    }
}

pub fn press_key(vk: u16) {
    if let Some(kc) = vk_keycode(vk) {
        let d = display_ptr();
        unsafe {
            XTestFakeKeyEvent(d, kc, 1, 0);
            XFlush(d);
        }
    }
}

pub fn release_key(vk: u16) {
    if let Some(kc) = vk_keycode(vk) {
        let d = display_ptr();
        unsafe {
            XTestFakeKeyEvent(d, kc, 0, 0);
            XFlush(d);
        }
    }
}

pub fn caps_lock_enabled() -> bool {
    let d = display_ptr();
    if d.is_null() {
        return false;
    }
    let mut state = XKeyboardState {
        key_click_percent: 0,
        bell_percent: 0,
        bell_pitch: 0,
        bell_duration: 0,
        led_mask: 0,
        global_auto_repeat: 0,
        auto_repeats: [0; 32],
    };
    unsafe { XGetKeyboardControl(d, &mut state) };
    (state.led_mask & 0x1) != 0
}

// -- hotkey polling ----------------------------------------------------------

/// Snapshot of currently pressed keyboard VK codes (physical layout, level 0).
pub fn pressed_vks() -> Vec<u16> {
    let d = display_ptr();
    if d.is_null() {
        return Vec::new();
    }
    let mut keymap = [0 as c_char; 32];
    unsafe { XQueryKeymap(d, keymap.as_mut_ptr()) };

    let mut vks = Vec::with_capacity(8);
    for keycode in 8u8..=255u8 {
        let byte = keymap[(keycode >> 3) as usize] as u8;
        let bit = 1u8 << (keycode & 7);
        if byte & bit != 0 {
            let keysym = unsafe { XKeycodeToKeysym(d, keycode as KeyCode, 0) };
            let vk = keysym_to_vk(keysym);
            if vk != 0 && !vks.contains(&vk) {
                vks.push(vk);
            }
        }
    }
    vks
}

pub fn is_key_name_pressed(name: &str) -> bool {
    let d = display_ptr();
    if d.is_null() {
        return false;
    }
    let c_name = match std::ffi::CString::new(name) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let keysym = unsafe { XStringToKeysym(c_name.as_ptr()) };
    if keysym == 0 {
        return false;
    }
    let keycode = unsafe { XKeysymToKeycode(d, keysym) };
    if keycode == 0 {
        return false;
    }
    let mut keymap = [0 as c_char; 32];
    unsafe { XQueryKeymap(d, keymap.as_mut_ptr()) };
    let idx = keycode as usize;
    (keymap[idx >> 3] as u8) & (1 << (idx & 7)) != 0
}

pub fn alt_down() -> bool {
    is_vk_pressed_now(VK_LMENU) || is_vk_pressed_now(VK_RMENU)
}

pub fn is_vk_pressed_now(vk: u16) -> bool {
    let d = display_ptr();
    if d.is_null() {
        return false;
    }
    let Some(keycode) = vk_keycode(vk) else {
        return false;
    };
    let mut keymap = [0 as c_char; 32];
    unsafe { XQueryKeymap(d, keymap.as_mut_ptr()) };
    let idx = keycode as usize;
    (keymap[idx >> 3] as u8) & (1 << (idx & 7)) != 0
}

fn reset_and_fill_storage(storage: &[AtomicBool; 256]) {
    for slot in storage {
        slot.store(false, Ordering::Relaxed);
    }
    for vk in pressed_vks() {
        if (vk as usize) < 256 {
            storage[vk as usize].store(true, Ordering::Relaxed);
        }
    }
    let mask = pointer_button_mask();
    storage[VK_LBUTTON as usize].store(mask & Button1Mask != 0, Ordering::Relaxed);
    storage[VK_MBUTTON as usize].store(mask & Button2Mask != 0, Ordering::Relaxed);
    storage[VK_RBUTTON as usize].store(mask & Button3Mask != 0, Ordering::Relaxed);
    // Generic modifier aliases mirror GetAsyncKeyState behaviour.
    let shift = storage[VK_LSHIFT as usize].load(Ordering::Relaxed)
        || storage[VK_RSHIFT as usize].load(Ordering::Relaxed);
    let ctrl = storage[VK_LCONTROL as usize].load(Ordering::Relaxed)
        || storage[VK_RCONTROL as usize].load(Ordering::Relaxed);
    let alt = storage[VK_LMENU as usize].load(Ordering::Relaxed)
        || storage[VK_RMENU as usize].load(Ordering::Relaxed);
    let super_down = storage[VK_LWIN as usize].load(Ordering::Relaxed)
        || storage[VK_RWIN as usize].load(Ordering::Relaxed);
    storage[VK_SHIFT as usize].store(shift, Ordering::Relaxed);
    storage[VK_CONTROL as usize].store(ctrl, Ordering::Relaxed);
    storage[VK_MENU as usize].store(alt, Ordering::Relaxed);
    if super_down {
        // no generic "super" code; sides already tracked
    }
}

/// Refresh the shared physical key state array (used by the hotkey listener
/// and pickers). Clearing every slot each poll keeps released keys accurate.
pub fn refresh_physical_state(storage: &[AtomicBool; 256]) {
    if is_connected() {
        reset_and_fill_storage(storage);
    }
}

// -- window queries ----------------------------------------------------------

const XA_UTF8_STRING: Atom = 45; // fallback pre-defined for UTF8_STRING
const None_Atom: Atom = 0;

fn intern_atom(name: &str) -> Atom {
    let d = display_ptr();
    if d.is_null() {
        return None_Atom;
    }
    let c_name = match std::ffi::CString::new(name) {
        Ok(c) => c,
        Err(_) => return None_Atom,
    };
    unsafe { XInternAtom(d, c_name.as_ptr(), 0) }
}

fn root_window() -> Window {
    let d = display_ptr();
    if d.is_null() {
        return 0;
    }
    unsafe { XDefaultRootWindow(d) }
}

unsafe fn get_property(
    d: *mut Display,
    w: Window,
    property: Atom,
) -> Option<(Atom, c_int, Vec<u8>)> {
    let mut actual_type: Atom = None_Atom;
    let mut actual_format: c_int = 0;
    let mut nitems: c_ulong = 0;
    let mut bytes_after: c_ulong = 0;
    let mut data: *mut c_uchar = std::ptr::null_mut();
    let status = XGetWindowProperty(
        d,
        w,
        property,
        0,
        1024,
        0,
        None_Atom,
        &mut actual_type,
        &mut actual_format,
        &mut nitems,
        &mut bytes_after,
        &mut data,
    );
    if status != 1 || data.is_null() || nitems == 0 {
        if !data.is_null() {
            XFree(data as *mut c_void);
        }
        return None;
    }
    let bytes = if actual_format == 32 {
        let item_size = std::mem::size_of::<c_ulong>();
        std::slice::from_raw_parts(data, (nitems as usize) * item_size).to_vec()
    } else {
        let len = (nitems as usize) * (actual_format as usize / 8).max(1);
        std::slice::from_raw_parts(data, len).to_vec()
    };
    XFree(data as *mut c_void);
    Some((actual_type, actual_format, bytes))
}

fn get_32bit_values(w: Window, property: Atom) -> Option<Vec<u64>> {
    let d = display_ptr();
    if d.is_null() || w == 0 {
        return None;
    }
    let (actual_type, format, data) = unsafe { get_property(d, w, property)? };
    if format != 32 || data.len() < 8 {
        return None;
    }
    let count = data.len() / 8;
    let mut values = Vec::with_capacity(count);
    for i in 0..count {
        let bytes: [u8; 8] = data[i * 8..i * 8 + 8].try_into().ok()?;
        values.push(u64::from_ne_bytes(bytes));
    }
    let _ = actual_type;
    Some(values)
}

fn get_string_value(w: Window, property: Atom) -> Option<String> {
    let d = display_ptr();
    if d.is_null() || w == 0 {
        return None;
    }
    let (_, _, data) = unsafe { get_property(d, w, property)? };
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    let text = String::from_utf8_lossy(&data[..end]).to_string();
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub fn window_title(w: Window) -> Option<String> {
    let d = display_ptr();
    if d.is_null() || w == 0 {
        return None;
    }
    let net_wm_name = intern_atom("_NET_WM_NAME");
    if let Some(title) = get_string_value(w, net_wm_name) {
        return Some(title);
    }
    // Fallback to legacy WM_NAME.
    let mut name_ptr: *mut c_char = std::ptr::null_mut();
    let status = unsafe { XFetchName(d, w, &mut name_ptr) };
    if status != 0 && !name_ptr.is_null() {
        let title = unsafe { std::ffi::CStr::from_ptr(name_ptr) }
            .to_string_lossy()
            .to_string();
        unsafe { XFree(name_ptr as *mut c_void) };
        let trimmed = title.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }
    None
}

pub fn window_pid(w: Window) -> Option<u32> {
    let net_wm_pid = intern_atom("_NET_WM_PID");
    let values = get_32bit_values(w, net_wm_pid)?;
    let pid = values.first().copied()?;
    if pid == 0 || pid > u32::MAX as u64 {
        return None;
    }
    Some(pid as u32)
}

pub fn active_window() -> Option<Window> {
    let root = root_window();
    if root == 0 {
        return None;
    }
    let net_active = intern_atom("_NET_ACTIVE_WINDOW");
    let values = get_32bit_values(root, net_active)?;
    values.first().copied().filter(|&w| w != 0 && w != 1).map(|w| w as Window)
}

pub fn active_window_pid() -> Option<u32> {
    window_pid(active_window()?)
}

pub fn collect_client_windows() -> Vec<Window> {
    let root = root_window();
    if root == 0 {
        return Vec::new();
    }
    let net_client = intern_atom("_NET_CLIENT_LIST");
    get_32bit_values(root, net_client)
        .unwrap_or_default()
        .into_iter()
        .filter(|&w| w != 0 && w != 1)
        .map(|w| w as Window)
        .collect()
}

/// Best-effort mapping of pid -> visible window title (longest recent title).
pub fn pid_title_map() -> std::collections::HashMap<u32, String> {
    let mut map: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
    let mut windows = collect_client_windows();
    if let Some(active) = active_window() {
        if !windows.contains(&active) {
            windows.push(active);
        }
    }
    for w in windows {
        let Some(pid) = window_pid(w) else { continue };
        let Some(title) = window_title(w) else { continue };
        let better = match map.get(&pid) {
            None => true,
            Some(existing) => {
                // Prefer the active/focused window's title, otherwise longer ones.
                if active_window() == Some(w) {
                    true
                } else {
                    title.chars().count() > existing.chars().count()
                }
            }
        };
        if better {
            map.insert(pid, title);
        }
    }
    map
}

// -- own-window detection ----------------------------------------------------

static OWN_WINDOW: AtomicU64 = AtomicU64::new(0);

pub fn set_own_window(window: u64) {
    OWN_WINDOW.store(window, Ordering::SeqCst);
}

fn pointer_child_window() -> Option<Window> {
    let d = display_ptr();
    if d.is_null() {
        return None;
    }
    let root = root_window();
    let mut rw: Window = 0;
    let mut child: Window = 0;
    let mut rx: c_int = 0;
    let mut ry: c_int = 0;
    let mut wx: c_int = 0;
    let mut wy: c_int = 0;
    let mut mask: c_uint = 0;
    unsafe {
        XQueryPointer(
            d,
            root,
            &mut rw,
            &mut child,
            &mut rx,
            &mut ry,
            &mut wx,
            &mut wy,
            &mut mask,
        )
    };
    if child == 0 {
        None
    } else {
        Some(child)
    }
}

fn is_ancestor_or_self(window: Window, target: Window) -> bool {
    if window == 0 || target == 0 {
        return false;
    }
    if window == target {
        return true;
    }
    let d = display_ptr();
    if d.is_null() {
        return false;
    }
    let mut current = target;
    for _ in 0..64 {
        let mut root: Window = 0;
        let mut parent: Window = 0;
        let mut children: *mut Window = std::ptr::null_mut();
        let mut nchildren: c_uint = 0;
        let status = unsafe {
            XQueryTree(
                d,
                current,
                &mut root,
                &mut parent,
                &mut children,
                &mut nchildren,
            )
        };
        if !children.is_null() {
            unsafe { XFree(children as *mut c_void) };
        }
        if status == 0 {
            return false;
        }
        if current == root {
            return false;
        }
        if parent == window {
            return true;
        }
        current = parent;
    }
    false
}

pub fn pointer_over_own_window() -> bool {
    // Windows checks whether the toplevel window under the cursor belongs to
    // our PID; mirror that on X11 by walking to the deepest window under the
    // pointer and then checking pid on it and its ancestors.
    let Some(mut window) = pointer_child_window() else {
        return false;
    };

    let own_pid = std::process::id();

    // Walk down to the deepest child containing the pointer.
    let d = display_ptr();
    if d.is_null() {
        return false;
    }
    loop {
        let mut child: Window = 0;
        let mut rw: Window = 0;
        let mut cx: c_int = 0;
        let mut cy: c_int = 0;
        let mut wx: c_int = 0;
        let mut wy: c_int = 0;
        let mut mask: c_uint = 0;
        unsafe {
            XQueryPointer(
                d,
                window,
                &mut rw,
                &mut child,
                &mut cx,
                &mut cy,
                &mut wx,
                &mut wy,
                &mut mask,
            )
        };
        if child == 0 || child == window {
            break;
        }
        window = child;
    }

    // Check the deepest window and each ancestor up to the toplevel.
    for _ in 0..64 {
        if window_pid(window) == Some(own_pid) {
            return true;
        }
        let Some(parent) = parent_window(window) else {
            break;
        };
        if parent == 0 {
            break;
        }
        window = parent;
    }
    false
}

fn parent_window(window: Window) -> Option<Window> {
    let d = display_ptr();
    if d.is_null() || window == 0 {
        return None;
    }
    let mut root: Window = 0;
    let mut parent: Window = 0;
    let mut children: *mut Window = std::ptr::null_mut();
    let mut nchildren: c_uint = 0;
    let status = unsafe {
        XQueryTree(
            d,
            window,
            &mut root,
            &mut parent,
            &mut children,
            &mut nchildren,
        )
    };
    if !children.is_null() {
        unsafe { XFree(children as *mut c_void) };
    }
    if status == 0 {
        return None;
    }
    Some(parent)
}

// -- X11 grab (pick-up-while-picking) ----------------------------------------

pub fn begin_pointer_redirect() -> bool {
    let d = display_ptr();
    if d.is_null() {
        return false;
    }
    let root = root_window();
    if root == 0 {
        return false;
    }
    let status = unsafe {
        XGrabPointer(
            d,
            root,
            0, // owner_events = false
            ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
            GrabModeAsync,
            GrabModeAsync,
            0, // confine_to = root (none)
            0, // cursor
            0, // CurrentTime
        )
    };
    unsafe { XSync(d, 0) };
    status == 0
}

pub fn end_pointer_redirect() {
    let d = display_ptr();
    if d.is_null() {
        return;
    }
    unsafe {
        XUngrabPointer(d, 0);
        XSync(d, 0);
    }
}

pub fn cleanup_this_thread() {
    close_thread_display();
}