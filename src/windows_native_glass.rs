use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::{null, null_mut};
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, Ellipse, EndPaint,
    GetStockObject, InvalidateRect, LineTo, MoveToEx, Rectangle, RoundRect, SelectObject,
    SetBkMode, SetTextColor, TextOutW, HOLLOW_BRUSH, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_TAB};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetSystemMetrics, LoadCursorW, PostQuitMessage, RegisterClassExW, SetForegroundWindow,
    SetLayeredWindowAttributes, SetWindowPos, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
    IDC_ARROW, LWA_ALPHA, LWA_COLORKEY, MSG, SM_CXSCREEN, SWP_SHOWWINDOW, SW_SHOW, WM_CHAR,
    WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_NCCREATE, WM_PAINT, WNDCLASSEXW,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

// Match the current macOS collapsed launcher geometry.
const WIDTH: i32 = 960;
const HEIGHT: i32 = 132;
const SEARCH_X: i32 = 24;
const SEARCH_Y: i32 = 22;
const SEARCH_W: i32 = WIDTH - SEARCH_X * 2;
const SEARCH_H: i32 = 68;
const BADGE_X: i32 = SEARCH_X + 18;
const BADGE_Y: i32 = SEARCH_Y + 15;
const BADGE_SIZE: i32 = 38;
const INPUT_X: i32 = BADGE_X + BADGE_SIZE + 16;
const HINT_X: i32 = WIDTH - 292;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeMode {
    Search,
    Clipboard,
}

#[derive(Debug)]
struct NativeState {
    mode: NativeMode,
    query: String,
    glass_hwnd: HWND,
    text_hwnd: HWND,
    input_hwnd: HWND,
}

unsafe impl Send for NativeState {}

static STATE: OnceLock<Mutex<NativeState>> = OnceLock::new();

pub fn run_prototype() {
    unsafe {
        let _ = STATE.set(Mutex::new(NativeState {
            mode: NativeMode::Search,
            query: String::new(),
            glass_hwnd: null_mut(),
            text_hwnd: null_mut(),
            input_hwnd: null_mut(),
        }));

        let glass_class = wide("ViceroyNativeMacLikeGlass");
        let text_class = wide("ViceroyNativeMacLikeText");
        let input_class = wide("ViceroyNativeMacLikeInput");
        let instance = GetModuleHandleA(null());

        register_class(instance, &glass_class, glass_wnd_proc);
        register_class(instance, &text_class, text_wnd_proc);
        register_class(instance, &input_class, input_wnd_proc);

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let x = ((screen_w - WIDTH).max(0)) / 2;
        let y = 164;

        let glass = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            glass_class.as_ptr(),
            wide("Viceroy Glass").as_ptr(),
            WS_POPUP,
            x,
            y,
            WIDTH,
            HEIGHT,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );
        if glass.is_null() {
            return;
        }

        let text = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            text_class.as_ptr(),
            wide("Viceroy Text").as_ptr(),
            WS_POPUP,
            x,
            y,
            WIDTH,
            HEIGHT,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );
        if text.is_null() {
            DestroyWindow(glass);
            return;
        }

        let input = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            input_class.as_ptr(),
            wide("Viceroy Input").as_ptr(),
            WS_POPUP,
            x,
            y,
            WIDTH,
            HEIGHT,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );
        if input.is_null() {
            DestroyWindow(text);
            DestroyWindow(glass);
            return;
        }

        if let Some(state) = STATE.get() {
            if let Ok(mut state) = state.lock() {
                state.glass_hwnd = glass;
                state.text_hwnd = text;
                state.input_hwnd = input;
            }
        }

        apply_composition(glass);
        SetLayeredWindowAttributes(glass, 0, 210, LWA_ALPHA);
        // Text window uses black as transparent color key, with fully opaque foreground pixels.
        SetLayeredWindowAttributes(text, 0x000000, 255, LWA_COLORKEY);
        // Invisible input layer captures clicks/keyboard across the whole glass surface.
        SetLayeredWindowAttributes(input, 0, 1, LWA_ALPHA);

        SetWindowPos(glass, null_mut(), x, y, WIDTH, HEIGHT, SWP_SHOWWINDOW);
        SetWindowPos(text, null_mut(), x, y, WIDTH, HEIGHT, SWP_SHOWWINDOW);
        SetWindowPos(input, null_mut(), x, y, WIDTH, HEIGHT, SWP_SHOWWINDOW);
        ShowWindow(glass, SW_SHOW);
        ShowWindow(text, SW_SHOW);
        ShowWindow(input, SW_SHOW);
        SetForegroundWindow(input);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe fn register_class(
    instance: *mut c_void,
    class_name: &[u16],
    wnd_proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
) {
    let wc = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: null_mut(),
        hCursor: LoadCursorW(null_mut(), IDC_ARROW),
        hbrBackground: GetStockObject(HOLLOW_BRUSH) as _,
        lpszMenuName: null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: null_mut(),
    };
    RegisterClassExW(&wc);
}

unsafe extern "system" fn glass_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => 1,
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            paint_glass(hwnd);
            0
        }
        WM_DESTROY => 0,
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn text_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => 1,
        WM_ERASEBKGND => 1,
        WM_LBUTTONDOWN => {
            SetForegroundWindow(hwnd);
            0
        }
        WM_KEYDOWN if wparam == VK_ESCAPE as usize => {
            close_windows();
            0
        }
        WM_KEYDOWN if wparam == VK_TAB as usize => {
            toggle_mode();
            0
        }
        WM_CHAR => {
            handle_char(wparam as u32);
            0
        }
        WM_PAINT => {
            paint_text(hwnd);
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn input_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => 1,
        WM_ERASEBKGND => 1,
        WM_LBUTTONDOWN => {
            SetForegroundWindow(hwnd);
            0
        }
        WM_KEYDOWN if wparam == VK_ESCAPE as usize => {
            close_windows();
            0
        }
        WM_KEYDOWN if wparam == VK_TAB as usize => {
            toggle_mode();
            0
        }
        WM_CHAR => {
            handle_char(wparam as u32);
            0
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            BeginPaint(hwnd, &mut ps);
            EndPaint(hwnd, &ps);
            0
        }
        WM_DESTROY => 0,
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn close_windows() {
    if let Some(state) = STATE.get() {
        if let Ok(state) = state.lock() {
            if !state.glass_hwnd.is_null() {
                DestroyWindow(state.glass_hwnd);
            }
            if !state.input_hwnd.is_null() {
                DestroyWindow(state.input_hwnd);
            }
            if !state.text_hwnd.is_null() {
                DestroyWindow(state.text_hwnd);
            }
        }
    }
    PostQuitMessage(0);
}

unsafe fn toggle_mode() {
    if let Some(state) = STATE.get() {
        if let Ok(mut state) = state.lock() {
            state.mode = match state.mode {
                NativeMode::Search => NativeMode::Clipboard,
                NativeMode::Clipboard => NativeMode::Search,
            };
            state.query.clear();
            InvalidateRect(state.text_hwnd, null(), 1);
        }
    }
}

unsafe fn handle_char(code: u32) {
    // Backspace is delivered as WM_CHAR 0x08. Ignore tab/escape here because WM_KEYDOWN handles them.
    if let Some(state) = STATE.get() {
        if let Ok(mut state) = state.lock() {
            match code {
                0x08 => {
                    state.query.pop();
                }
                0x09 | 0x1b | 0x0d => {}
                _ => {
                    if let Some(ch) = char::from_u32(code) {
                        if !ch.is_control() {
                            state.query.push(ch);
                        }
                    }
                }
            }
            InvalidateRect(state.text_hwnd, null(), 1);
        }
    }
}

unsafe fn paint_glass(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);

    // Overall faint shell, then the darker macOS search capsule inside it.
    let shell_brush = CreateSolidBrush(0x004c4638);
    let old_brush = SelectObject(hdc, shell_brush as _);
    let shell_pen = CreatePen(PS_SOLID, 1, 0x006c6254);
    let old_pen = SelectObject(hdc, shell_pen as _);
    RoundRect(hdc, 0, 0, WIDTH, HEIGHT, 48, 48);

    let search_brush = CreateSolidBrush(0x00171614);
    let search_pen = CreatePen(PS_SOLID, 1, 0x003a3836);
    SelectObject(hdc, search_brush as _);
    SelectObject(hdc, search_pen as _);
    RoundRect(
        hdc,
        SEARCH_X,
        SEARCH_Y,
        SEARCH_X + SEARCH_W,
        SEARCH_Y + SEARCH_H,
        40,
        40,
    );

    let badge_brush = CreateSolidBrush(0x00312f2d);
    SelectObject(hdc, badge_brush as _);
    SelectObject(hdc, GetStockObject(HOLLOW_BRUSH));
    RoundRect(
        hdc,
        BADGE_X,
        BADGE_Y,
        BADGE_X + BADGE_SIZE,
        BADGE_Y + BADGE_SIZE,
        26,
        26,
    );

    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    DeleteObject(shell_pen as _);
    DeleteObject(search_pen as _);
    DeleteObject(shell_brush as _);
    DeleteObject(search_brush as _);
    DeleteObject(badge_brush as _);
    EndPaint(hwnd, &ps);
}

unsafe fn paint_text(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);

    // Clear previous glyphs with the transparent colorkey color. Without this,
    // repeated repaints can leave placeholder/query text visually stacked.
    let clear_brush = CreateSolidBrush(0x00000000);
    let clear_pen = CreatePen(PS_SOLID, 1, 0x00000000);
    let old_brush = SelectObject(hdc, clear_brush as _);
    let old_pen = SelectObject(hdc, clear_pen as _);
    Rectangle(hdc, 0, 0, WIDTH, HEIGHT);
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    DeleteObject(clear_pen as _);
    DeleteObject(clear_brush as _);

    SetBkMode(hdc, TRANSPARENT as i32);

    let (mode, query) = STATE
        .get()
        .and_then(|state| {
            state
                .lock()
                .ok()
                .map(|state| (state.mode, state.query.clone()))
        })
        .unwrap_or((NativeMode::Search, String::new()));

    draw_magnifier(hdc);
    let placeholder = match mode {
        NativeMode::Search => "Search apps, files, clipboard history, and more",
        NativeMode::Clipboard => "Filter clipboard history",
    };
    let input = if query.is_empty() {
        placeholder
    } else {
        &query
    };
    let color = if query.is_empty() {
        0x00d2d7df
    } else {
        0x00f7f8fb
    };
    draw_text(hdc, input, INPUT_X, SEARCH_Y, color, -20);

    let hint = match mode {
        NativeMode::Search => "Tab: Clipboard   Esc: Hide",
        NativeMode::Clipboard => "Tab: Search   Esc: Hide",
    };
    draw_text(hdc, hint, HINT_X, SEARCH_Y, 0x00b8bfca, -14);

    EndPaint(hwnd, &ps);
}

unsafe fn draw_magnifier(hdc: windows_sys::Win32::Graphics::Gdi::HDC) {
    let pen = CreatePen(PS_SOLID, 2, 0x00c5c9d0);
    let old_pen = SelectObject(hdc, pen as _);
    SelectObject(hdc, GetStockObject(HOLLOW_BRUSH));
    let cx = BADGE_X + 18;
    let cy = BADGE_Y + 17;
    Ellipse(hdc, cx - 7, cy - 7, cx + 7, cy + 7);
    MoveToEx(hdc, cx + 6, cy + 6, null_mut());
    LineTo(hdc, cx + 13, cy + 13);
    SelectObject(hdc, old_pen);
    DeleteObject(pen as _);
}

unsafe fn draw_text(
    hdc: windows_sys::Win32::Graphics::Gdi::HDC,
    text: &str,
    x: i32,
    search_y: i32,
    color: u32,
    font_height: i32,
) {
    let font = CreateFontW(
        font_height,
        0,
        0,
        0,
        450,
        0,
        0,
        0,
        0,
        0,
        0,
        5,
        0,
        wide("Segoe UI Variable").as_ptr(),
    );
    let old_font = SelectObject(hdc, font as _);
    SetTextColor(hdc, color);
    let text = wide(text);
    let y = search_y + ((SEARCH_H - font_height.abs()).max(0) / 2) - 2;
    let len = text.len().saturating_sub(1) as i32;
    TextOutW(hdc, x, y, text.as_ptr(), len);
    SelectObject(hdc, old_font);
    if !font.is_null() {
        DeleteObject(font as _);
    }
}

unsafe fn apply_composition(hwnd: HWND) {
    let corner_pref: i32 = DWMWCP_ROUND;
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_WINDOW_CORNER_PREFERENCE as u32,
        &corner_pref as *const _ as *const c_void,
        size_of::<i32>() as u32,
    );

    let dark_mode: i32 = 1;
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
        &dark_mode as *const _ as *const c_void,
        size_of::<i32>() as u32,
    );

    let backdrop: i32 = DWMSBT_TRANSIENTWINDOW;
    let _ = DwmSetWindowAttribute(
        hwnd,
        DWMWA_SYSTEMBACKDROP_TYPE as u32,
        &backdrop as *const _ as *const c_void,
        size_of::<i32>() as u32,
    );

    apply_acrylic(hwnd);
}

#[repr(C)]
struct AccentPolicy {
    accent_state: i32,
    accent_flags: i32,
    gradient_color: u32,
    animation_id: i32,
}

#[repr(C)]
struct WindowCompositionAttribData {
    attribute: i32,
    data: *mut c_void,
    size_of_data: usize,
}

#[allow(clippy::manual_c_str_literals)]
unsafe fn apply_acrylic(hwnd: HWND) {
    const WCA_ACCENT_POLICY: i32 = 19;
    const ACCENT_ENABLE_ACRYLICBLURBEHIND: i32 = 4;

    let user32 = GetModuleHandleA(b"user32.dll\0".as_ptr());
    if user32.is_null() {
        return;
    }

    let Some(raw_fn) = GetProcAddress(user32, b"SetWindowCompositionAttribute\0".as_ptr()) else {
        return;
    };

    type SetWindowCompositionAttributeFn =
        unsafe extern "system" fn(HWND, *mut WindowCompositionAttribData) -> i32;
    let set_window_composition_attribute: SetWindowCompositionAttributeFn =
        core::mem::transmute(raw_fn);

    let mut accent = AccentPolicy {
        accent_state: ACCENT_ENABLE_ACRYLICBLURBEHIND,
        accent_flags: 0,
        gradient_color: 40 | (44 << 8) | (54 << 16) | (0x52 << 24),
        animation_id: 0,
    };
    let mut data = WindowCompositionAttribData {
        attribute: WCA_ACCENT_POLICY,
        data: &mut accent as *mut _ as *mut c_void,
        size_of_data: size_of::<AccentPolicy>(),
    };
    let _ = set_window_composition_attribute(hwnd, &mut data);
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
