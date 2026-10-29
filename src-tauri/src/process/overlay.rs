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

use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{COLORREF, HWND as WinHWND, RECT as WinRECT, TRUE},
        Graphics::Gdi::{
            CreateBitmap, CreateCompatibleDC, CreateDIBSection, CreateFontW as WinCreateFontW,
            CreateSolidBrush as WinCreateSolidBrush, DeleteDC, DeleteObject as DeleteGdiObject,
            DrawTextW as WinDrawTextW, Ellipse as WinEllipse, GetStockObject,
            SelectObject as WinSelectObject, SetBkMode as WinSetBkMode,
            SetTextColor as WinSetTextColor, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
            CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET as WIN_DEFAULT_CHARSET,
            DEFAULT_PITCH, DIB_RGB_COLORS, DT_CENTER as WIN_DT_CENTER,
            DT_END_ELLIPSIS as WIN_DT_END_ELLIPSIS, DT_SINGLELINE as WIN_DT_SINGLELINE,
            DT_VCENTER as WIN_DT_VCENTER, FF_DONTCARE, HGDIOBJ as WinHGDIOBJ, NULL_PEN,
            OUT_DEFAULT_PRECIS, TRANSPARENT as WIN_TRANSPARENT,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED,
        },
        UI::Shell::{ITaskbarList3, TaskbarList},
        UI::WindowsAndMessaging::{CreateIconIndirect, DestroyIcon, HICON, ICONINFO},
    },
};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::{
        BeginPaint, ClientToScreen, CreateFontW, CreateRoundRectRgn, CreateSolidBrush,
        DeleteObject, DrawTextW, EndPaint, FillRect, InvalidateRect, SelectObject, SetBkMode,
        SetTextColor, SetWindowRgn, UpdateWindow, DEFAULT_CHARSET, DT_CENTER, DT_END_ELLIPSIS,
        DT_SINGLELINE, DT_VCENTER, HGDIOBJ, PAINTSTRUCT, TRANSPARENT,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect,
        GetCursorPos, GetWindowLongPtrW, GetWindowRect, IsIconic, IsWindow, IsWindowVisible,
        PeekMessageW, RegisterClassW, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos,
        ShowWindow, TranslateMessage, GWLP_USERDATA, LWA_ALPHA, LWA_COLORKEY, MSG, PM_REMOVE,
        SWP_NOACTIVATE, SWP_NOZORDER, SW_HIDE, SW_SHOWNOACTIVATE, WNDCLASSW, WS_EX_LAYERED,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
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
    taskbar: Option<TaskbarBadge>,
}

struct TaskbarBadge {
    list: ITaskbarList3,
    icon: HICON,
    description: Vec<u16>,
    applied: bool,
}

impl Drop for TaskbarBadge {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyIcon(self.icon);
        }
    }
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

pub fn spawn(pid: u32, name: String, badge_text: String, color: String, border_width: u32) {
    let text = truncate(&format!("FinEnv · {name}"), 28);
    thread::spawn(move || run(pid, text, badge_text, color, border_width));
}

fn run(pid: u32, text: String, badge_text: String, color: String, border_width: u32) {
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
    let com_initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok() };
    let taskbar = if com_initialized {
        unsafe { create_taskbar_badge(&text, &badge_text, background) }
    } else {
        None
    };
    let frame_state = Box::new(Frame {
        color: background,
        color_key,
        width: 0,
        height: 0,
        thickness: border_width.min(12) as i32,
        taskbar,
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
        if com_initialized {
            unsafe { CoUninitialize() };
        }
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
                        if width < 260 || height < 260 {
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
        if com_initialized {
            CoUninitialize();
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
    // Keep the taskbar badge active even when the window is too narrow for the
    // in-window border and floating badge.
    apply_taskbar_overlay(state, chrome);
    // Chromium's outer window rect includes the invisible Windows resize border
    // (8px on the sides and bottom at 100% DPI). Anchoring the overlay to the
    // client rect keeps the coloured frame flush with the visible page instead
    // of leaving a gap around the left, right and bottom edges.
    let mut client = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if GetClientRect(chrome, &mut client) == 0 {
        ShowWindow(frame, SW_HIDE);
        return;
    }
    let mut origin = POINT { x: 0, y: 0 };
    if ClientToScreen(chrome, &mut origin) == 0 {
        ShowWindow(frame, SW_HIDE);
        return;
    }
    let width = client.right - client.left;
    let height = client.bottom - client.top;
    if width < 180 || height < 180 {
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
        origin.x,
        origin.y,
        width,
        height,
        SWP_NOZORDER | SWP_NOACTIVATE,
    );
    ShowWindow(frame, SW_SHOWNOACTIVATE);
}

unsafe fn apply_taskbar_overlay(state: &mut Frame, chrome: HWND) {
    if let Some(taskbar) = state.taskbar.as_mut() {
        if !taskbar.applied {
            let result = taskbar.list.SetOverlayIcon(
                WinHWND(chrome),
                taskbar.icon,
                PCWSTR(taskbar.description.as_ptr()),
            );
            if result.is_ok() {
                taskbar.applied = true;
            }
        }
    }
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

fn window_area(hwnd: HWND) -> i64 {
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
        return 0;
    }
    i64::from(rect.right - rect.left).max(0) * i64::from(rect.bottom - rect.top).max(0)
}

fn wait_for_window(pid: u32) -> Option<HWND> {
    for _ in 0..100 {
        // Chromium may briefly create auxiliary top-level windows (for example
        // the "restore pages?" bubble). Pick the largest window so the border
        // and badge always attach to the actual browser frame.
        if let Some(handle) = super::window_handles(pid)
            .into_iter()
            .filter(|hwnd| window_area(*hwnd) >= 180 * 180)
            .max_by_key(|hwnd| window_area(*hwnd))
        {
            return Some(handle);
        }
        thread::sleep(Duration::from_millis(200));
    }
    None
}

unsafe fn create_taskbar_badge(name: &str, badge_text: &str, color: u32) -> Option<TaskbarBadge> {
    let list: ITaskbarList3 = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok()?;
    list.HrInit().ok()?;
    let icon = create_overlay_icon(color, badge_text.trim())?;
    let description = if badge_text.trim().is_empty() {
        name.to_string()
    } else {
        format!("{} · {name}", badge_text.trim())
    };
    Some(TaskbarBadge {
        list,
        icon,
        description: wide(&description),
        applied: false,
    })
}

unsafe fn create_overlay_icon(color: u32, badge_text: &str) -> Option<HICON> {
    const SIZE: i32 = 32;
    const CENTER: f32 = 15.5;
    const OUTER_SQ: f32 = 15.0 * 15.0;

    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: SIZE,
            biHeight: -SIZE,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut std::ffi::c_void = null_mut();
    let color_bitmap = CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
    if bits.is_null() {
        let _ = DeleteGdiObject(WinHGDIOBJ(color_bitmap.0));
        return None;
    }

    let hdc = CreateCompatibleDC(None);
    if hdc.is_invalid() {
        let _ = DeleteGdiObject(WinHGDIOBJ(color_bitmap.0));
        return None;
    }
    let old_bitmap = WinSelectObject(hdc, WinHGDIOBJ(color_bitmap.0));
    let old_pen = WinSelectObject(hdc, GetStockObject(NULL_PEN));

    let white = WinCreateSolidBrush(COLORREF(0x00FF_FFFF));
    let fill = WinCreateSolidBrush(COLORREF(color));
    WinSelectObject(hdc, WinHGDIOBJ(white.0));
    let _ = WinEllipse(hdc, 0, 0, SIZE, SIZE);
    WinSelectObject(hdc, WinHGDIOBJ(fill.0));
    let _ = WinEllipse(hdc, 2, 2, SIZE - 2, SIZE - 2);

    if !badge_text.is_empty() {
        let count = badge_text.chars().count();
        let has_wide_glyph = !badge_text.is_ascii();
        let font_height = match (count, has_wide_glyph) {
            (1, true) => 23,
            (2, true) => 16,
            (3, true) => 13,
            (_, true) => 11,
            (1, false) => 24,
            (2, false) => 19,
            (3, false) => 15,
            _ => 12,
        };
        let font = WinCreateFontW(
            -font_height,
            0,
            0,
            0,
            700,
            0,
            0,
            0,
            WIN_DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            (FF_DONTCARE.0 | DEFAULT_PITCH.0) as u32,
            w!("Microsoft YaHei"),
        );
        let old_font = WinSelectObject(hdc, WinHGDIOBJ(font.0));
        WinSetBkMode(hdc, WIN_TRANSPARENT);
        WinSetTextColor(hdc, COLORREF(0x00FF_FFFF));
        let mut text: Vec<u16> = badge_text.encode_utf16().collect();
        let mut rect = WinRECT {
            left: 0,
            top: 0,
            right: SIZE,
            bottom: SIZE,
        };
        WinDrawTextW(
            hdc,
            &mut text,
            &mut rect,
            WIN_DT_CENTER | WIN_DT_VCENTER | WIN_DT_SINGLELINE | WIN_DT_END_ELLIPSIS,
        );
        WinSelectObject(hdc, old_font);
        let _ = DeleteGdiObject(WinHGDIOBJ(font.0));
    }

    let _ = DeleteGdiObject(WinHGDIOBJ(white.0));
    let _ = DeleteGdiObject(WinHGDIOBJ(fill.0));
    WinSelectObject(hdc, old_pen);
    WinSelectObject(hdc, old_bitmap);
    let _ = DeleteDC(hdc);

    let pixels = std::slice::from_raw_parts_mut(bits as *mut u8, (SIZE * SIZE * 4) as usize);
    let mut mask_bits = vec![0u8; (SIZE * SIZE / 8) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - CENTER;
            let dy = y as f32 - CENTER;
            let distance = dx * dx + dy * dy;
            let offset = ((y * SIZE + x) * 4) as usize;
            if distance <= OUTER_SQ {
                pixels[offset + 3] = 255;
            } else {
                pixels[offset..offset + 4].fill(0);
                let mask_index = (y * (SIZE / 8) + x / 8) as usize;
                mask_bits[mask_index] |= 1 << (7 - (x % 8));
            }
        }
    }

    let mask_bitmap = CreateBitmap(
        SIZE,
        SIZE,
        1,
        1,
        Some(mask_bits.as_ptr() as *const std::ffi::c_void),
    );
    if mask_bitmap.is_invalid() {
        let _ = DeleteGdiObject(WinHGDIOBJ(color_bitmap.0));
        return None;
    }
    let icon = CreateIconIndirect(&ICONINFO {
        fIcon: TRUE,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: mask_bitmap,
        hbmColor: color_bitmap,
    })
    .ok();
    let _ = DeleteGdiObject(WinHGDIOBJ(color_bitmap.0));
    let _ = DeleteGdiObject(WinHGDIOBJ(mask_bitmap.0));
    icon
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
