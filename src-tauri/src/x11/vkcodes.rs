// Global VK code integers (Windows virtual-key definitions) and the small set
// of Win32 window-message constants that the pure classification logic
// references. The values match winuser.h so hotkey/picker behaviour is
// identical across platforms; individual platforms only differ in how key
// state is sampled.
#![allow(dead_code)]

pub const VK_LBUTTON: u16 = 0x01;
pub const VK_RBUTTON: u16 = 0x02;
pub const VK_CANCEL: u16 = 0x03;
pub const VK_MBUTTON: u16 = 0x04;
pub const VK_XBUTTON1: u16 = 0x05;
pub const VK_XBUTTON2: u16 = 0x06;
pub const VK_BACK: u16 = 0x08;
pub const VK_TAB: u16 = 0x09;
pub const VK_CLEAR: u16 = 0x0C;
pub const VK_RETURN: u16 = 0x0D;
pub const VK_SHIFT: u16 = 0x10;
pub const VK_CONTROL: u16 = 0x11;
pub const VK_MENU: u16 = 0x12;
pub const VK_PAUSE: u16 = 0x13;
pub const VK_CAPITAL: u16 = 0x14;
pub const VK_KANA: u16 = 0x15;
pub const VK_ESCAPE: u16 = 0x1B;
pub const VK_SPACE: u16 = 0x20;
pub const VK_PRIOR: u16 = 0x21;
pub const VK_NEXT: u16 = 0x22;
pub const VK_END: u16 = 0x23;
pub const VK_HOME: u16 = 0x24;
pub const VK_LEFT: u16 = 0x25;
pub const VK_UP: u16 = 0x26;
pub const VK_RIGHT: u16 = 0x27;
pub const VK_DOWN: u16 = 0x28;
pub const VK_SNAPSHOT: u16 = 0x2C;
pub const VK_INSERT: u16 = 0x2D;
pub const VK_DELETE: u16 = 0x2E;
pub const VK_HELP: u16 = 0x2F;

pub const VK_0: u16 = 0x30;
pub const VK_9: u16 = 0x39;

pub const VK_A: u16 = 0x41;
pub const VK_Z: u16 = 0x5A;

pub const VK_LWIN: u16 = 0x5B;
pub const VK_RWIN: u16 = 0x5C;
pub const VK_APPS: u16 = 0x5D;

pub const VK_NUMPAD0: u16 = 0x60;
pub const VK_NUMPAD1: u16 = 0x61;
pub const VK_NUMPAD2: u16 = 0x62;
pub const VK_NUMPAD3: u16 = 0x63;
pub const VK_NUMPAD4: u16 = 0x64;
pub const VK_NUMPAD5: u16 = 0x65;
pub const VK_NUMPAD6: u16 = 0x66;
pub const VK_NUMPAD7: u16 = 0x67;
pub const VK_NUMPAD8: u16 = 0x68;
pub const VK_NUMPAD9: u16 = 0x69;
pub const VK_MULTIPLY: u16 = 0x6A;
pub const VK_ADD: u16 = 0x6B;
pub const VK_SEPARATOR: u16 = 0x6C;
pub const VK_SUBTRACT: u16 = 0x6D;
pub const VK_DECIMAL: u16 = 0x6E;
pub const VK_DIVIDE: u16 = 0x6F;

pub const VK_F1: u16 = 0x70;
pub const VK_F24: u16 = 0x87;

pub const VK_NUMLOCK: u16 = 0x90;
pub const VK_SCROLL: u16 = 0x91;

pub const VK_LSHIFT: u16 = 0xA0;
pub const VK_RSHIFT: u16 = 0xA1;
pub const VK_LCONTROL: u16 = 0xA2;
pub const VK_RCONTROL: u16 = 0xA3;
pub const VK_LMENU: u16 = 0xA4;
pub const VK_RMENU: u16 = 0xA5;

pub const VK_OEM_1: u16 = 0xBA; // ;:
pub const VK_OEM_PLUS: u16 = 0xBB; // =+
pub const VK_OEM_COMMA: u16 = 0xBC; // ,<
pub const VK_OEM_MINUS: u16 = 0xBD; // -_
pub const VK_OEM_PERIOD: u16 = 0xBE; // .>
pub const VK_OEM_2: u16 = 0xBF; // /?
pub const VK_OEM_3: u16 = 0xC0; // `~
pub const VK_OEM_4: u16 = 0xDB; // [{
pub const VK_OEM_5: u16 = 0xDC; // \|
pub const VK_OEM_6: u16 = 0xDD; // ]}
pub const VK_OEM_7: u16 = 0xDE; // '"
pub const VK_OEM_8: u16 = 0xDF;
pub const VK_OEM_102: u16 = 0xE2; // non-US \| / <>

// Win32 window-message codes used purely as classification constants.
pub const WM_MOUSEMOVE: u32 = 0x0200;
pub const WM_LBUTTONDOWN: u32 = 0x0201;
pub const WM_LBUTTONUP: u32 = 0x0202;
pub const WM_RBUTTONDOWN: u32 = 0x0204;
pub const WM_RBUTTONUP: u32 = 0x0205;
pub const WM_RBUTTONDBLCLK: u32 = 0x0206;
pub const WM_MBUTTONDOWN: u32 = 0x0207;
pub const WM_MBUTTONUP: u32 = 0x0208;
pub const WM_XBUTTONDOWN: u32 = 0x020B;
pub const WM_XBUTTONUP: u32 = 0x020C;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_KEYUP: u32 = 0x0101;
pub const WM_SYSKEYDOWN: u32 = 0x0104;
pub const WM_QUIT: u32 = 0x0012;
pub const WM_SETICON: u32 = 0x0080;
