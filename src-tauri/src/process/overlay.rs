//! Windows-only floating identity badge drawn over the Chromium tab strip.
//!
//! Chrome does not expose the empty caption area to web content, and its
//! "window name" is only shown in the taskbar/window menu. To make the active
//! environment unmistakable we draw a small click-through, owner-bound pill
//! over the tab strip and keep it aligned with the browser window.

use std::ptr::{null, null_mut};
use std::sync::Once;
use std::thread;
use std::time::Duration;

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::{
        BeginPaint, CreateFontW, CreateRoundRectRgn, CreateSolidBrush, DeleteObject, DrawTextW,
        EndPaint, FillRect, InvalidateRect, SelectObject, SetBkMode, SetTextColor, SetWindowRgn,
        UpdateWindow, DEFAULT_CHARSET, DT_CENTER, DT_END_ELLIPSIS, DT_SINGLELINE, DT_VCENTER,
        HGDIOBJ, PAINTSTRUCT, TRANSPARENT,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos,
        GetWindowLongPtrW, GetWindowRect, IsIconic, IsWindow, IsWindowVisible, PeekMessageW,
        RegisterClassW, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindow,
        TranslateMessage, GWLP_USERDATA, LWA_ALPHA, LWA_COLORKEY, MSG, PM_REMOVE, SWP_NOACTIVATE,
        SWP_NOZORDER, SW_HIDE, SW_SHOWNOACTIVATE, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
    },
};

const BADGE_WIDTH: i32 = 230;
const BADGE_HEIGHT: i32 = 28;
const BADGE_MARGIN: i32 = 24;

struct Badge {
    text: Vec<u16>,
    background: u32,
    foreground: u32,
    font: HGDIOBJ,
}

struct Frame {
    color: u32,
    color_key: u32,
    width: i32,
    height: i32,
    thickness: i32,
}

impl Drop for Badge {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.font);
        }
    }
}

static REGISTER_CLASS: Once = Once::new();
static REGISTER_FRAME_CLASS: Once = Once::new();

pub fn spawn(pid: u32, name: String, color: String, border_width: u32) {
    let text = truncate(&format!("FinEnv · {name}"), 28);
    thread::spawn(move || run(pid, text, color, border_width));
}

fn run(pid: u32, text: String, color: String, border_width: u32) {
    let chrome = wait_for_window(pid);
    let Some(chrome) = chrome else {
        return;
    };
    REGISTER_CLASS.call_once(|| unsafe {
        let class_name = wide("FinEnvEnvironmentBadge");
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(null()),
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class_name.as_ptr(),
        };
        RegisterClassW(&wc);
    });
    REGISTER_FRAME_CLASS.call_once(|| unsafe {
        let class_name = wide("FinEnvEnvironmentFrame");
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(frame_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(null()),
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class_name.as_ptr(),
        };
        RegisterClassW(&wc);
    });

    let background = parse_color(&color).unwrap_or(0x00A3_3426);
    let (r, g, b) = (
        background & 0xFF,
        (background >> 8) & 0xFF,
        (background >> 16) & 0xFF,
    );
    let luminance = 0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64;
    let foreground = if luminance > 165.0 {
        0x0010_1A12
    } else {
        0x00FF_FFFF
    };
    let font = unsafe {
        CreateFontW(
            -13,
            0,
            0,
            0,
            600,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            0,
            0,
            wide("Segoe UI").as_ptr(),
        )
    };
    let badge_state = Box::new(Badge {
        text: wide(&text),
        background,
        foreground,
        font: font as HGDIOBJ,
    });
    let state_ptr = Box::into_raw(badge_state);

    let class_name = wide("FinEnvEnvironmentBadge");
    let window_name = wide(&text);
    let badge = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
            class_name.as_ptr(),
            window_name.as_ptr(),
            WS_POPUP,
            0,
            0,
            BADGE_WIDTH,
            BADGE_HEIGHT,
            chrome,
            null_mut(),
            GetModuleHandleW(null()),
            state_ptr as *const _,
        )
    };
    if badge.is_null() {
        drop(unsafe { Box::from_raw(state_ptr) });
        return;
    }
    unsafe {
        SetWindowLongPtrW(badge, GWLP_USERDATA, state_ptr as isize);
        let region = CreateRoundRectRgn(0, 0, BADGE_WIDTH + 1, BADGE_HEIGHT + 1, 14, 14);
        if !region.is_null() {
            SetWindowRgn(badge, region, 1);
        }
        ShowWindow(badge, SW_SHOWNOACTIVATE);
        SetLayeredWindowAttributes(badge, 0, 235, LWA_ALPHA);
        UpdateWindow(badge);
    }

    let color_key = if background == 0x00FF_00FF {
        0x00FE_00FE
    } else {
        0x00FF_00FF
    };
    let frame_state = Box::new(Frame {
        color: background,
        color_key,
        width: 0,
        height: 0,
        thickness: border_width.min(12) as i32,
    });
    let frame_ptr = Box::into_raw(frame_state);
    let frame_class = wide("FinEnvEnvironmentFrame");
    let frame_name = wide("FinEnv Environment Border");
    let frame = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
            frame_class.as_ptr(),
            frame_name.as_ptr(),
            WS_POPUP,
            0,
            0,
            1,
            1,
            chrome,
            null_mut(),
            GetModuleHandleW(null()),
            frame_ptr as *const _,
        )
    };
    if frame.is_null() {
        drop(unsafe { Box::from_raw(frame_ptr) });
    } else {
        unsafe {
            SetWindowLongPtrW(frame, GWLP_USERDATA, frame_ptr as isize);
            SetLayeredWindowAttributes(frame, color_key, 0, LWA_COLORKEY);
            ShowWindow(frame, SW_HIDE);
        }
    }

    let mut message: MSG = unsafe { std::mem::zeroed() };
    let mut tick = 0u32;
    loop {
        unsafe {
            while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        if unsafe { IsWindow(chrome) } == 0 || unsafe { IsWindow(badge) } == 0 {
            break;
        }
        if tick % 5 == 0 {
            unsafe {
                if IsIconic(chrome) != 0 || IsWindowVisible(chrome) == 0 {
                    ShowWindow(badge, SW_HIDE);
                } else {
                    let mut rect = RECT {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    };
                    if GetWindowRect(chrome, &mut rect) != 0 {
                        let width = rect.right - rect.left;
                        let height = rect.bottom - rect.top;
                        if width < 640 || height < 420 {
                            ShowWindow(badge, SW_HIDE);
                        } else {
                            let x = rect.right - BADGE_WIDTH - BADGE_MARGIN;
                            let y = rect.bottom - BADGE_HEIGHT - BADGE_MARGIN;
                            SetWindowPos(
                                badge,
                                null_mut(),
                                x,
                                y,
                                BADGE_WIDTH,
                                BADGE_HEIGHT,
                                SWP_NOZORDER | SWP_NOACTIVATE,
                            );
                            let mut cursor = POINT { x: 0, y: 0 };
                            let hovered = GetCursorPos(&mut cursor) != 0
                                && cursor.x >= x
                                && cursor.x <= x + BADGE_WIDTH
                                && cursor.y >= y
                                && cursor.y <= y + BADGE_HEIGHT;
                            SetLayeredWindowAttributes(
                                badge,
                                0,
                                if hovered { 55 } else { 235 },
                                LWA_ALPHA,
                            );
                            ShowWindow(badge, SW_SHOWNOACTIVATE);
                        }
                    }
                }
            }
        }
        unsafe {
            frame_sync(frame, frame_ptr, chrome);
        }
        tick = tick.wrapping_add(1);
        thread::sleep(Duration::from_millis(60));
    }

    unsafe {
        SetWindowLongPtrW(badge, GWLP_USERDATA, 0);
        drop(Box::from_raw(state_ptr));
        if !frame.is_null() {
            DestroyWindow(frame);
            drop(Box::from_raw(frame_ptr));
        }
    }
}

unsafe fn frame_sync(frame: HWND, frame_ptr: *mut Frame, chrome: HWND) {
    if frame.is_null() || frame_ptr.is_null() {
        return;
    }
    let state = &mut *frame_ptr;
    if state.thickness <= 0 || IsIconic(chrome) != 0 || IsWindowVisible(chrome) == 0 {
        ShowWindow(frame, SW_HIDE);
        return;
    }
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if GetWindowRect(chrome, &mut rect) == 0 {
        ShowWindow(frame, SW_HIDE);
        return;
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width < 640 || height < 420 {
        ShowWindow(frame, SW_HIDE);
        return;
    }
    if state.width != width || state.height != height {
        state.width = width;
        state.height = height;
        InvalidateRect(frame, null(), 1);
    }
    SetWindowPos(
        frame,
        null_mut(),
        rect.left,
        rect.top,
        width,
        height,
        SWP_NOZORDER | SWP_NOACTIVATE,
    );
    ShowWindow(frame, SW_SHOWNOACTIVATE);
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    const WM_PAINT: u32 = 0x000F;
    const WM_ERASEBKGND: u32 = 0x0014;

    match message {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Badge;
            if pointer.is_null() {
                return DefWindowProcW(hwnd, message, wparam, lparam);
            }
            let badge = &*pointer;
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut paint);
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: BADGE_WIDTH,
                bottom: BADGE_HEIGHT,
            };
            let brush = CreateSolidBrush(badge.background);
            FillRect(hdc, &rect, brush);
            DeleteObject(brush as HGDIOBJ);
            SetBkMode(hdc, TRANSPARENT as i32);
            SetTextColor(hdc, badge.foreground);
            let previous = SelectObject(hdc, badge.font);
            DrawTextW(
                hdc,
                badge.text.as_ptr(),
                -1,
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
            );
            SelectObject(hdc, previous);
            EndPaint(hwnd, &paint);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe extern "system" fn frame_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    const WM_PAINT: u32 = 0x000F;
    const WM_ERASEBKGND: u32 = 0x0014;

    match message {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Frame;
            if pointer.is_null() {
                return DefWindowProcW(hwnd, message, wparam, lparam);
            }
            let frame = &*pointer;
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut paint);
            let width = frame.width.max(1);
            let height = frame.height.max(1);
            let thickness = frame.thickness.clamp(0, 12).min(width / 2).min(height / 2);
            let key_brush = CreateSolidBrush(frame.color_key);
            FillRect(
                hdc,
                &RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                },
                key_brush,
            );
            DeleteObject(key_brush as HGDIOBJ);
            if thickness > 0 {
                let brush = CreateSolidBrush(frame.color);
                for rect in [
                    RECT {
                        left: 0,
                        top: 0,
                        right: width,
                        bottom: thickness,
                    },
                    RECT {
                        left: 0,
                        top: height - thickness,
                        right: width,
                        bottom: height,
                    },
                    RECT {
                        left: 0,
                        top: 0,
                        right: thickness,
                        bottom: height,
                    },
                    RECT {
                        left: width - thickness,
                        top: 0,
                        right: width,
                        bottom: height,
                    },
                ] {
                    FillRect(hdc, &rect, brush);
                }
                DeleteObject(brush as HGDIOBJ);
            }
            EndPaint(hwnd, &paint);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

fn wait_for_window(pid: u32) -> Option<HWND> {
    for _ in 0..100 {
        if let Some(handle) = super::window_handles(pid).into_iter().next() {
            return Some(handle);
        }
        thread::sleep(Duration::from_millis(200));
    }
    None
}

fn parse_color(value: &str) -> Option<u32> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u32::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u32::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u32::from_str_radix(&hex[4..6], 16).ok()?;
    Some(r | (g << 8) | (b << 16))
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut out: String = value.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
