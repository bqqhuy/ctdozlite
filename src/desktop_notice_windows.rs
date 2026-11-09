//! Native topmost popup and taskbar overlay, owned by the tray message thread.
use crate::desktop_notice::{self, Notice, Popup};
use std::{
    cell::{Cell, RefCell},
    sync::Once,
    time::{Duration, Instant},
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{Com::*, LibraryLoader::*},
        UI::{HiDpi::GetDpiForWindow, Shell::*, WindowsAndMessaging::*},
    },
};
thread_local! {
    static CARD:RefCell<Option<Notice>>=const{RefCell::new(None)};
    static CLOSE:Cell<bool>=const{Cell::new(false)};
}
fn wide(s: &str) -> Vec<u16> {
    s.chars()
        .filter(|c| *c != '\0')
        .take(1024)
        .collect::<String>()
        .encode_utf16()
        .chain(Some(0))
        .collect()
}
unsafe fn text(dc: HDC, text: &str, mut rect: RECT, size: i32, bold: bool, color: COLORREF) {
    let font = CreateFontW(
        size,
        0,
        0,
        0,
        if bold { 600 } else { 400 },
        0,
        0,
        0,
        1,
        0,
        0,
        5,
        0,
        w!("Segoe UI"),
    );
    let old = SelectObject(dc, font);
    SetTextColor(dc, color);
    SetBkMode(dc, TRANSPARENT);
    let mut utf = wide(text);
    utf.pop();
    DrawTextW(
        dc,
        &mut utf,
        &mut rect,
        DT_SINGLELINE | DT_END_ELLIPSIS | DT_VCENTER | DT_NOPREFIX,
    );
    SelectObject(dc, old);
    DeleteObject(font);
}
unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_ERASEBKGND => LRESULT(1),
        WM_LBUTTONUP => {
            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);
            let x = (lp.0 as u32 & 65535) as i16 as i32;
            let scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
            let card = CARD.with(|slot| slot.try_borrow().ok().and_then(|v| v.clone()));
            CLOSE.with(|c| c.set(true));
            ShowWindow(hwnd, SW_HIDE);
            if x < rect.right - (28.0 * scale) as i32 {
                if let Some(notice) = card {
                    crate::tray::open_notice(notice);
                }
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut ps);
            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);
            let brush = CreateSolidBrush(COLORREF(0x00ffffff));
            FillRect(dc, &rect, brush);
            DeleteObject(brush);
            let border = CreateSolidBrush(COLORREF(0x00d4d7de));
            FrameRect(dc, &rect, border);
            DeleteObject(border);
            let s = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
            let px = |n: f32| (n * s).round() as i32;
            let card = CARD.with(|slot| slot.try_borrow().ok().and_then(|v| v.clone()));
            if let Some(n) = card {
                let brush = CreateSolidBrush(COLORREF(0x00ff6800));
                let old = SelectObject(dc, brush);
                Ellipse(dc, px(8.0), px(14.0), px(60.0), px(66.0));
                SelectObject(dc, old);
                DeleteObject(brush);
                let initials = n.title.chars().next().unwrap_or('Z').to_string();
                text(
                    dc,
                    &initials,
                    RECT {
                        left: px(25.0),
                        top: px(14.0),
                        right: px(55.0),
                        bottom: px(65.0),
                    },
                    px(22.0),
                    true,
                    COLORREF(0x00ffffff),
                );
                if let Some(avatar) = desktop_notice::avatar(&n) {
                    let info = BITMAPINFO {
                        bmiHeader: BITMAPINFOHEADER {
                            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                            biWidth: avatar.width as i32,
                            biHeight: -(avatar.height as i32),
                            biPlanes: 1,
                            biBitCount: 32,
                            biCompression: BI_RGB.0,
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    let region = CreateEllipticRgn(px(8.0), px(14.0), px(60.0), px(66.0));
                    SelectClipRgn(dc, region);
                    StretchDIBits(
                        dc,
                        px(8.0),
                        px(14.0),
                        px(52.0),
                        px(52.0),
                        0,
                        0,
                        avatar.width as i32,
                        avatar.height as i32,
                        Some(avatar.bgra.as_ptr().cast()),
                        &info,
                        DIB_RGB_COLORS,
                        SRCCOPY,
                    );
                    SelectClipRgn(dc, HRGN::default());
                    DeleteObject(region);
                }
                text(
                    dc,
                    &n.title,
                    RECT {
                        left: px(68.0),
                        top: px(12.0),
                        right: rect.right - px(25.0),
                        bottom: px(36.0),
                    },
                    px(16.0),
                    true,
                    COLORREF(0x00332715),
                );
                text(
                    dc,
                    &n.body,
                    RECT {
                        left: px(68.0),
                        top: px(40.0),
                        right: rect.right - px(10.0),
                        bottom: px(65.0),
                    },
                    px(14.0),
                    false,
                    COLORREF(0x0069815a),
                );
            }
            text(
                dc,
                "×",
                RECT {
                    left: rect.right - px(23.0),
                    top: 0,
                    right: rect.right - px(2.0),
                    bottom: px(24.0),
                },
                px(18.0),
                false,
                COLORREF(0x00998c7f),
            );
            EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_CLOSE => {
            CLOSE.with(|c| c.set(true));
            ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn overlay(count: usize) -> HICON {
    if count == 0 {
        return HICON::default();
    }
    let mut pixels = vec![0u8; 32 * 32 * 4];
    for y in 0..32i32 {
        for x in 0..32i32 {
            if (x - 16).pow(2) + (y - 16).pow(2) <= 15i32.pow(2) {
                let p = ((y * 32 + x) * 4) as usize;
                pixels[p..p + 4].copy_from_slice(&[45, 38, 220, 255]);
            }
        }
    }
    let text = desktop_notice::badge(count);
    let glyphs: [[u8; 5]; 6] = [
        [2, 6, 2, 2, 7],
        [7, 1, 7, 4, 7],
        [7, 1, 7, 1, 7],
        [5, 5, 7, 1, 1],
        [7, 4, 7, 1, 7],
        [0, 2, 7, 2, 0],
    ];
    let scale = 3usize;
    let total = text.len() * 4 * scale - scale;
    let start = (32 - total) / 2;
    for (i, c) in text.bytes().enumerate() {
        let g = if c == b'+' { 5 } else { (c - b'1') as usize };
        for (y, row) in glyphs[g].iter().enumerate() {
            for x in 0..3 {
                if row & (1 << (2 - x)) != 0 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let p = ((8 + y * scale + dy) * 32
                                + start
                                + i * 4 * scale
                                + x * scale
                                + dx)
                                * 4;
                            pixels[p..p + 4].copy_from_slice(&[255; 4]);
                        }
                    }
                }
            }
        }
    }
    // SAFETY: bitmap data and ICONINFO are live until CreateIconIndirect copies them.
    unsafe {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: 32,
                biHeight: -32,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let Ok(color) = CreateDIBSection(HDC(0), &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
            return HICON::default();
        };
        if bits.is_null() {
            DeleteObject(color);
            return HICON::default();
        }
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len());
        let mask_bits = [0u8; 128];
        let mask = CreateBitmap(32, 32, 1, 1, Some(mask_bits.as_ptr().cast()));
        let info = ICONINFO {
            fIcon: BOOL(1),
            hbmColor: color,
            hbmMask: mask,
            ..Default::default()
        };
        let icon = CreateIconIndirect(&info).unwrap_or_default();
        DeleteObject(color);
        DeleteObject(mask);
        icon
    }
}

pub(crate) struct Desktop {
    hwnd: HWND,
    root: HWND,
    taskbar: Option<ITaskbarList3>,
    icon: HICON,
    count: Option<usize>,
    popup: Popup,
    last_sound: Option<Instant>,
    last_overlay: Instant,
}
impl Desktop {
    pub fn new(root: isize) -> anyhow::Result<Self> {
        // SAFETY: COM and all window/GDI resources stay on this message-loop thread.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            static CLASS: Once = Once::new();
            CLASS.call_once(|| {
                let class = WNDCLASSW {
                    lpfnWndProc: Some(procedure),
                    hInstance: GetModuleHandleW(None).unwrap_or_default().into(),
                    lpszClassName: w!("ctdozlite.MessagePopup"),
                    hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                    ..Default::default()
                };
                RegisterClassW(&class);
            });
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
                w!("ctdozlite.MessagePopup"),
                w!("Tin nhắn mới"),
                WS_POPUP,
                0,
                0,
                350,
                80,
                HWND(0),
                None,
                GetModuleHandleW(None)?,
                None,
            );
            anyhow::ensure!(hwnd.0 != 0, "Không tạo được cửa sổ native");
            let taskbar: Option<ITaskbarList3> =
                CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok();
            if let Some(t) = &taskbar {
                let _ = t.HrInit();
            }
            Ok(Self {
                hwnd,
                root: HWND(root),
                taskbar,
                icon: HICON::default(),
                count: None,
                popup: Popup::default(),
                last_sound: None,
                last_overlay: Instant::now() - Duration::from_secs(3),
            })
        }
    }
    pub fn tick(&mut self, background: bool) {
        let snapshot = desktop_notice::snapshot();
        let now = Instant::now();
        // SAFETY: this object owns its HWND/HICON and all calls run on their owner thread.
        unsafe {
            if self.count != Some(snapshot.count)
                || self.last_overlay.elapsed() >= Duration::from_secs(2)
            {
                self.last_overlay = now;
                let icon = overlay(snapshot.count);
                let description = wide(&format!("{} tin chưa đọc", snapshot.count));
                // Explorer may replace the COM server; retry on a fresh interface.
                // Record the attempted count so an unavailable shell is retried every
                // two seconds rather than on every popup animation tick.
                self.count = Some(snapshot.count);
                let applied = self.taskbar.as_ref().is_some_and(|t| {
                    t.SetOverlayIcon(self.root, icon, PCWSTR(description.as_ptr()))
                        .is_ok()
                });
                if !applied {
                    self.taskbar = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok();
                    if let Some(t) = &self.taskbar {
                        let _ = t.HrInit();
                        let _ = t.SetOverlayIcon(self.root, icon, PCWSTR(description.as_ptr()));
                    }
                }
                if self.icon.0 != 0 {
                    let _ = DestroyIcon(self.icon);
                }
                self.icon = icon;
            }
            let new_notice = snapshot.sequence != self.popup.seen;
            if CLOSE.with(|c| c.replace(false)) {
                self.popup.close();
            }
            let mut pointer = POINT::default();
            let mut bounds = RECT::default();
            if IsWindowVisible(self.hwnd).as_bool()
                && GetCursorPos(&mut pointer).is_ok()
                && GetWindowRect(self.hwnd, &mut bounds).is_ok()
                && pointer.x >= bounds.left
                && pointer.x < bounds.right
                && pointer.y >= bounds.top
                && pointer.y < bounds.bottom
            {
                self.popup.hold(now);
            }
            self.popup.update(&snapshot, now, background);
            CARD.with(|slot| {
                if let Ok(mut slot) = slot.try_borrow_mut() {
                    *slot = self.popup.current.clone();
                }
            });
            if new_notice
                && snapshot.notice.is_some()
                && snapshot.enabled
                && !snapshot.privacy_locked
                && snapshot.sound
                && background
                && self
                    .last_sound
                    .is_none_or(|t| now.saturating_duration_since(t) >= Duration::from_millis(500))
            {
                self.last_sound = Some(now);
                crate::notification_sound::play();
            }
            if self.popup.current.is_none() {
                ShowWindow(self.hwnd, SW_HIDE);
                return;
            }
            let monitor = MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            GetMonitorInfoW(monitor, &mut info);
            let scale = GetDpiForWindow(self.hwnd).max(96) as f32 / 96.0;
            let width = (350.0 * scale) as i32;
            let height = (80.0 * scale) as i32;
            let exit = self
                .popup
                .exit_at
                .map(|t| (now.saturating_duration_since(t).as_secs_f32() / 0.15).min(1.0))
                .unwrap_or(0.0);
            let enter = self
                .popup
                .shown_at
                .map(|t| (now.saturating_duration_since(t).as_secs_f32() / 0.15).min(1.0))
                .unwrap_or(1.0);
            let alpha = ((1.0 - exit) * enter * 255.0) as u8;
            let _ = SetLayeredWindowAttributes(self.hwnd, COLORREF(0), alpha, LWA_ALPHA);
            let dy = ((exit + 1.0 - enter) * 24.0 * scale) as i32;
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                info.rcWork.right - width - (5.0 * scale) as i32,
                info.rcWork.bottom - height - (5.0 * scale) as i32 + dy,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            InvalidateRect(self.hwnd, None, false);
        }
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        // SAFETY: only resources owned by this message thread are destroyed.
        unsafe {
            if let Some(t) = &self.taskbar {
                let _ = t.SetOverlayIcon(self.root, HICON::default(), w!(""));
            }
            if self.icon.0 != 0 {
                let _ = DestroyIcon(self.icon);
            }
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_popup_is_topmost_nonactivating_and_close_button_hides_it() {
        let desktop = Desktop::new(0).expect("create native popup");
        // SAFETY: all windows/GDI objects belong to this test thread.
        unsafe {
            let style = GetWindowLongPtrW(desktop.hwnd, GWL_EXSTYLE) as u32;
            assert_ne!(style & WS_EX_TOPMOST.0, 0);
            assert_ne!(style & WS_EX_NOACTIVATE.0, 0);
            assert_ne!(style & WS_EX_TOOLWINDOW.0, 0);
            let foreground = GetForegroundWindow();
            SetWindowPos(
                desktop.hwnd,
                HWND_TOPMOST,
                0,
                0,
                350,
                80,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
            .unwrap();
            assert!(IsWindowVisible(desktop.hwnd).as_bool());
            assert_eq!(GetForegroundWindow(), foreground);
            assert_eq!(
                SendMessageW(desktop.hwnd, WM_MOUSEACTIVATE, WPARAM(0), LPARAM(0)).0,
                MA_NOACTIVATE as isize
            );
            let icon = overlay(42);
            assert_ne!(icon.0, 0);
            DestroyIcon(icon).unwrap();
            SendMessageW(desktop.hwnd, WM_LBUTTONUP, WPARAM(0), LPARAM(342));
            assert!(!IsWindowVisible(desktop.hwnd).as_bool());
            assert!(CLOSE.with(|c| c.replace(false)));
        }
    }
}
