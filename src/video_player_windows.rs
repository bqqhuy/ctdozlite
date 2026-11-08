//! MFPlay renders and synchronizes audio/video in a native, worker-owned window.
use anyhow::{Context, Result};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Once,
    },
    time::{Duration, Instant},
};
use windows::{
    core::{w, PCWSTR, PROPVARIANT},
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        Media::MediaFoundation::*,
        System::{Com::*, LibraryLoader::*},
        UI::WindowsAndMessaging::*,
    },
};
static GENERATION: AtomicU64 = AtomicU64::new(0);
pub(crate) static ACTIVE_VIDEO_WINDOW: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
pub(crate) static REQUESTED_SEEK_100NS: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(-1);
pub(crate) static REQUESTED_VOLUME: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(u32::MAX);
const WM_APP_SEEK: u32 = 0x8000 + 1;
const WM_APP_SET_VOL: u32 = 0x8000 + 2;
thread_local! {
    static PLAYER:RefCell<Option<IMFPMediaPlayer>>=const{RefCell::new(None)};
    static ROOT:Cell<HWND>=const{Cell::new(HWND(0))};
    static SURFACE:Cell<HWND>=const{Cell::new(HWND(0))};
    static FULL:Cell<bool>=const{Cell::new(false)};
    static DRAG:Cell<bool>=const{Cell::new(false)};
    static RESUME:Cell<bool>=const{Cell::new(false)};
    static TARGET:Cell<i64>=const{Cell::new(0)};
    static VOLUME:Cell<f32>=const{Cell::new(1.0)};
    static FONT:Cell<HFONT>=const{Cell::new(HFONT(0))};
    static FONT_DPI:Cell<i32>=const{Cell::new(0)};

}
struct Runtime;
impl Runtime {
    fn start() -> Result<Self> {
        // SAFETY: hold System32 media modules for process lifetime before delay imports.
        unsafe {
            for module in ["mfplat.dll", "mf.dll", "mfplay.dll"] {
                let wide: Vec<u16> = module.encode_utf16().chain(Some(0)).collect();
                LoadLibraryExW(PCWSTR(wide.as_ptr()), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
                    .context("Windows thiếu Media Feature Pack")?;
            }
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
            if let Err(error) = MFStartup(MF_VERSION, MFSTARTUP_FULL) {
                CoUninitialize();
                return Err(error.into());
            }
        }
        Ok(Self)
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        // SAFETY: balances startup on this media-window thread.
        unsafe {
            let _ = MFShutdown();
            CoUninitialize();
        }
    }
}
unsafe fn player() -> Option<IMFPMediaPlayer> {
    PLAYER.with(|p| p.try_borrow().ok().and_then(|p| p.clone()))
}
unsafe fn seek(delta: i64) {
    if let Some(p) = player() {
        if let Ok(position) = p.GetPosition(&MFP_POSITIONTYPE_100NS) {
            if let Ok(at) = i64::try_from(&position) {
                let duration = p
                    .GetDuration(&MFP_POSITIONTYPE_100NS)
                    .ok()
                    .and_then(|v| i64::try_from(&v).ok())
                    .unwrap_or(i64::MAX);
                let target = at.saturating_add(delta).clamp(0, duration.max(0));
                let _ = p.SetPosition(&MFP_POSITIONTYPE_100NS, &PROPVARIANT::from(target));
            }
        }
    }
}
#[link(name = "user32")]
extern "system" {
    fn SetCapture(hwnd: HWND) -> HWND;
    fn ReleaseCapture() -> BOOL;
    fn SetFocus(hwnd: HWND) -> HWND;
}
unsafe fn dpi(hwnd: HWND) -> i32 {
    windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd).max(96) as i32
}
unsafe fn timeline(hwnd: HWND) -> RECT {
    let mut r = RECT::default();
    let _ = GetClientRect(hwnd, &mut r);
    let unit = dpi(hwnd) as f32 / 96.0;
    RECT {
        left: (16.0 * unit) as i32,
        right: (r.right - (16.0 * unit) as i32).max(17),
        top: r.bottom - (94.0 * unit) as i32,
        bottom: r.bottom - (72.0 * unit) as i32,
    }
}
unsafe fn position() -> (i64, i64) {
    player()
        .map(|p| {
            (
                p.GetPosition(&MFP_POSITIONTYPE_100NS)
                    .ok()
                    .and_then(|v| i64::try_from(&v).ok())
                    .unwrap_or(0),
                p.GetDuration(&MFP_POSITIONTYPE_100NS)
                    .ok()
                    .and_then(|v| i64::try_from(&v).ok())
                    .unwrap_or(0),
            )
        })
        .unwrap_or_default()
}
fn time_label(ticks: i64) -> String {
    let seconds = ticks.max(0) / 10_000_000;
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}
unsafe fn target_at(hwnd: HWND, x: i32) -> i64 {
    let r = timeline(hwnd);
    let (_, duration) = position();
    ((x - r.left).clamp(0, (r.right - r.left).max(1)) as f64 / (r.right - r.left).max(1) as f64
        * duration.max(0) as f64) as i64
}
unsafe fn set_label(hwnd: HWND, id: i32, label: &str) {
    let text: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
    let _ = SetWindowTextW(GetDlgItem(hwnd, id), PCWSTR(text.as_ptr()));
}
unsafe fn layout(hwnd: HWND) {
    let mut r = RECT::default();
    let _ = GetClientRect(hwnd, &mut r);
    let window_dpi = dpi(hwnd);
    let unit = window_dpi as f32 / 96.0;
    if FONT_DPI.with(Cell::get) != window_dpi {
        let mut spec = LOGFONTW {
            lfHeight: -(14 * window_dpi / 96),
            lfWeight: 400,
            ..Default::default()
        };
        for (out, ch) in spec.lfFaceName.iter_mut().zip("Segoe UI".encode_utf16()) {
            *out = ch;
        }
        let font = CreateFontIndirectW(&spec);
        if font.0 != 0 {
            for id in 1..=8 {
                let _ = SendMessageW(
                    GetDlgItem(hwnd, id),
                    WM_SETFONT,
                    WPARAM(font.0 as usize),
                    LPARAM(1),
                );
            }
            let old = FONT.with(|f| f.replace(font));
            if old.0 != 0 {
                DeleteObject(old);
            }
            FONT_DPI.with(|d| d.set(window_dpi));
        }
    }
    let video = SURFACE.with(Cell::get);
    if video.0 != 0 {
        let _ = MoveWindow(
            video,
            0,
            0,
            r.right,
            (r.bottom - (108.0 * unit) as i32).max(1),
            true,
        );
    }
    let width = ((r.right - 16).max(7) / 7 - 4).max(1);
    for index in 0..7 {
        let button = GetDlgItem(hwnd, index + 1);
        if button.0 != 0 {
            let _ = MoveWindow(
                button,
                8 + index * (width + 4),
                r.bottom - (38.0 * unit) as i32,
                width,
                (30.0 * unit) as i32,
                true,
            );
        }
    }
    let _ = MoveWindow(
        GetDlgItem(hwnd, 8),
        16,
        r.bottom - (69.0 * unit) as i32,
        (r.right - 32).max(1),
        (25.0 * unit) as i32,
        true,
    );
    if let Some(p) = player() {
        let _ = p.UpdateVideo();
    }
}
unsafe fn refresh_controls(hwnd: HWND) {
    let (at, duration) = position();
    let playing = player().is_some_and(|p| {
        p.GetState()
            .is_ok_and(|s| s == MFP_MEDIAPLAYER_STATE_PLAYING)
    });
    set_label(hwnd, 1, if playing { "Tạm dừng" } else { "Phát" });
    let volume = player().and_then(|p| p.GetVolume().ok()).unwrap_or(0.0);
    set_label(
        hwnd,
        7,
        if volume <= 0.001 {
            "Bật tiếng"
        } else {
            "Tắt tiếng"
        },
    );
    let mut pointer = POINT::default();
    let _ = GetCursorPos(&mut pointer);
    let _ = ScreenToClient(hwnd, &mut pointer);
    let r = timeline(hwnd);
    let hover =
        pointer.x >= r.left && pointer.x <= r.right && pointer.y >= r.top && pointer.y <= r.bottom;
    let display = if DRAG.with(Cell::get) {
        TARGET.with(Cell::get)
    } else {
        at
    };
    let detail = if DRAG.with(Cell::get) || hover {
        format!(
            " • Tua đến {}",
            time_label(if hover {
                target_at(hwnd, pointer.x)
            } else {
                display
            })
        )
    } else {
        String::new()
    };
    set_label(
        hwnd,
        8,
        &if duration > 0 {
            format!(
                "{} / {} • Âm lượng {:.0}%{}",
                time_label(display),
                time_label(duration),
                volume * 100.0,
                detail
            )
        } else {
            "Đang nạp video…".into()
        },
    );
    let _ = InvalidateRect(hwnd, Some(&r), false);
}
unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_GETMINMAXINFO if lp.0 != 0 => {
            // SAFETY: Windows owns this writable structure for the message callback.
            let info = &mut *(lp.0 as *mut MINMAXINFO);
            let scale = dpi(hwnd);
            info.ptMinTrackSize = POINT {
                x: 480 * scale / 96,
                y: 320 * scale / 96,
            };
            LRESULT(0)
        }
        WM_DPICHANGED => {
            if hwnd == ROOT.with(Cell::get) && lp.0 != 0 {
                // SAFETY: Windows supplies a suggested RECT for the duration of this message.
                let r = *(lp.0 as *const RECT);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                layout(hwnd);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            if hwnd == ROOT.with(Cell::get) {
                layout(hwnd);
            }
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let dc = BeginPaint(hwnd, &mut ps);
            if hwnd == SURFACE.with(Cell::get) {
                if let Some(p) = player() {
                    let _ = p.UpdateVideo();
                }
            } else {
                let mut r = RECT::default();
                let _ = GetClientRect(hwnd, &mut r);
                let brush = CreateSolidBrush(COLORREF(0x002b2622));
                FillRect(dc, &r, brush);
                DeleteObject(brush);
                let bar = timeline(hwnd);
                let y = (bar.top + bar.bottom) / 2;
                let track = RECT {
                    left: bar.left,
                    right: bar.right,
                    top: y - 3,
                    bottom: y + 3,
                };
                let gray = CreateSolidBrush(COLORREF(0x00605a54));
                FillRect(dc, &track, gray);
                DeleteObject(gray);
                let (at, duration) = position();
                let at = if DRAG.with(Cell::get) {
                    TARGET.with(Cell::get)
                } else {
                    at
                };
                let right = if duration > 0 {
                    bar.left
                        + ((bar.right - bar.left) as f64
                            * (at as f64 / duration as f64).clamp(0.0, 1.0))
                            as i32
                } else {
                    bar.left
                };
                let fill = RECT { right, ..track };
                let blue = CreateSolidBrush(COLORREF(0x00ff6800));
                FillRect(dc, &fill, blue);
                let dot = RECT {
                    left: right - 4,
                    right: right + 4,
                    top: y - 7,
                    bottom: y + 7,
                };
                FillRect(dc, &dot, blue);
                DeleteObject(blue);
            }
            EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            let dc = HDC(wp.0 as isize);
            SetTextColor(dc, COLORREF(0x00ffffff));
            SetBkMode(dc, TRANSPARENT);
            LRESULT(GetStockObject(BLACK_BRUSH).0)
        }
        WM_LBUTTONDOWN if hwnd == ROOT.with(Cell::get) => {
            let x = lp.0 as i16 as i32;
            let y = (lp.0 >> 16) as i16 as i32;
            let r = timeline(hwnd);
            if x >= r.left && x <= r.right && y >= r.top && y <= r.bottom && position().1 > 0 {
                DRAG.with(|s| s.set(true));
                TARGET.with(|s| s.set(target_at(hwnd, x)));
                if let Some(p) = player() {
                    let playing = p
                        .GetState()
                        .is_ok_and(|s| s == MFP_MEDIAPLAYER_STATE_PLAYING);
                    RESUME.with(|s| s.set(playing));
                    let _ = p.Pause();
                }
                SetCapture(hwnd);
                refresh_controls(hwnd);
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE if hwnd == ROOT.with(Cell::get) => {
            if DRAG.with(Cell::get) {
                TARGET.with(|s| s.set(target_at(hwnd, lp.0 as i16 as i32)));
            }
            refresh_controls(hwnd);
            LRESULT(0)
        }
        WM_LBUTTONUP if hwnd == ROOT.with(Cell::get) => {
            if DRAG.with(|s| s.replace(false)) {
                ReleaseCapture();
                if let Some(p) = player() {
                    let _ = p.SetPosition(
                        &MFP_POSITIONTYPE_100NS,
                        &PROPVARIANT::from(TARGET.with(Cell::get)),
                    );
                    if RESUME.with(Cell::get) {
                        let _ = p.Play();
                    }
                }
                refresh_controls(hwnd);
            }
            LRESULT(0)
        }
        WM_CAPTURECHANGED => {
            if DRAG.with(|s| s.replace(false)) && RESUME.with(Cell::get) {
                if let Some(p) = player() {
                    let _ = p.Play();
                }
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            match wp.0 & 65535 {
                1 => {
                    if let Some(p) = player() {
                        if p.GetState()
                            .is_ok_and(|s| s == MFP_MEDIAPLAYER_STATE_PLAYING)
                        {
                            let _ = p.Pause();
                        } else {
                            let _ = p.Play();
                        }
                    }
                }
                2 => seek(-100_000_000),
                3 => seek(100_000_000),
                4 | 5 => {
                    if let Some(p) = player() {
                        if let Ok(v) = p.GetVolume() {
                            let _ = p.SetVolume(
                                (v + if wp.0 & 65535 == 4 { -0.1 } else { 0.1 }).clamp(0.0, 1.0),
                            );
                        }
                    }
                }
                6 => FULL.with(|s| s.set(!s.get())),
                7 => {
                    if let Some(p) = player() {
                        if let Ok(v) = p.GetVolume() {
                            if v > 0.001 {
                                VOLUME.with(|s| s.set(v));
                                let _ = p.SetVolume(0.0);
                            } else {
                                let _ = p.SetVolume(VOLUME.with(Cell::get).max(0.1));
                            }
                        }
                    }
                }
                _ => {}
            }
            refresh_controls(ROOT.with(Cell::get));
            let _ = SetFocus(ROOT.with(Cell::get));
            LRESULT(0)
        }
        WM_KEYDOWN => {
            match wp.0 {
                0x1b => {
                    if FULL.with(Cell::get) {
                        FULL.with(|s| s.set(false));
                    } else {
                        let _ = DestroyWindow(ROOT.with(Cell::get));
                    }
                }
                0x20 => {
                    let _ = PostMessageW(ROOT.with(Cell::get), WM_COMMAND, WPARAM(1), LPARAM(0));
                }
                0x25 => seek(-100_000_000),
                0x27 => seek(100_000_000),
                0x7a => FULL.with(|s| s.set(!s.get())),
                _ => {}
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            ACTIVE_VIDEO_WINDOW.store(0, Ordering::SeqCst);
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            ACTIVE_VIDEO_WINDOW.store(0, Ordering::SeqCst);
            if hwnd == ROOT.with(Cell::get) {
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        WM_APP_SEEK => {
            let target = REQUESTED_SEEK_100NS.swap(-1, Ordering::SeqCst);
            if target >= 0 {
                if let Some(p) = player() {
                    let _ = p.SetPosition(&MFP_POSITIONTYPE_100NS, &PROPVARIANT::from(target));
                }
            }
            refresh_controls(ROOT.with(Cell::get));
            LRESULT(0)
        }
        WM_APP_SET_VOL => {
            let raw = REQUESTED_VOLUME.swap(u32::MAX, Ordering::SeqCst);
            if raw <= 1000 {
                let vol = raw as f32 / 1000.0;
                if let Some(p) = player() {
                    let _ = p.SetVolume(vol);
                }
            }
            refresh_controls(ROOT.with(Cell::get));
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
struct WindowGuard(HWND);
impl Drop for WindowGuard {
    fn drop(&mut self) {
        // SAFETY: the guard and COM player remain on the creating thread.
        unsafe {
            PLAYER.with(|p| {
                if let Some(p) = p.borrow_mut().take() {
                    let _ = p.Shutdown();
                }
            });
            if IsWindow(self.0).as_bool() {
                let _ = DestroyWindow(self.0);
            }
            let font = FONT.with(|f| f.replace(HFONT(0)));
            if font.0 != 0 {
                DeleteObject(font);
            }
            FONT_DPI.with(|d| d.set(0));
            ROOT.with(|s| s.set(HWND(0)));
            SURFACE.with(|s| s.set(HWND(0)));
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, WM_QUIT, WM_QUIT, PM_REMOVE).as_bool() {}
        }
    }
}
pub(super) fn run(path: &Path, title: &str, active: impl Fn() -> bool) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let generation = GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    let _runtime = Runtime::start()?;
    let url: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    anyhow::ensure!(
        !url[..url.len() - 1].contains(&0),
        "Đường dẫn video không hợp lệ"
    );
    let title: Vec<u16> = format!("{title} — Trình xem video")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // SAFETY: all HWNDs, callbacks, player interfaces and COM stay on this thread.
    unsafe {
        static CLASS: Once = Once::new();
        CLASS.call_once(|| {
            let c = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: GetModuleHandleW(None).unwrap_or_default().into(),
                lpszClassName: w!("ctdozlite.VideoViewer"),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                ..Default::default()
            };
            RegisterClassW(&c);
        });
        let root = CreateWindowExW(
            WS_EX_APPWINDOW,
            w!("ctdozlite.VideoViewer"),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            100,
            100,
            960,
            640,
            None,
            None,
            GetModuleHandleW(None)?,
            None,
        );
        anyhow::ensure!(root.0 != 0, "Không tạo được cửa sổ native");
        let _window = WindowGuard(root);
        ROOT.with(|s| s.set(root));
        ACTIVE_VIDEO_WINDOW.store(root.0, Ordering::SeqCst);
        FULL.with(|s| s.set(false));
        let surface = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("ctdozlite.VideoViewer"),
            w!(""),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            940,
            540,
            root,
            None,
            GetModuleHandleW(None)?,
            None,
        );
        anyhow::ensure!(surface.0 != 0, "Không tạo được cửa sổ native");
        SURFACE.with(|s| s.set(surface));
        let mut media = None;
        if let Err(error) = MFPCreateMediaPlayer(
            PCWSTR(url.as_ptr()),
            true,
            MFP_CREATION_OPTIONS(0),
            None,
            surface,
            Some(&mut media),
        ) {
            let _ = DestroyWindow(root);
            return Err(error.into());
        }
        let media = media.context("Không tạo được trình phát video")?;
        let _ = media.SetAspectRatioMode(MFVideoARMode_PreservePicture.0 as u32);
        PLAYER.with(|p| *p.borrow_mut() = Some(media));
        for (index, label) in [
            "Phát / Tạm dừng",
            "−10 giây",
            "+10 giây",
            "Âm −",
            "Âm +",
            "Toàn màn",
            "Tắt tiếng",
        ]
        .iter()
        .enumerate()
        {
            let label: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                PCWSTR(label.as_ptr()),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                0,
                0,
                100,
                32,
                root,
                HMENU(index as isize + 1),
                GetModuleHandleW(None)?,
                None,
            );
        }
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("Đang nạp video…"),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            300,
            24,
            root,
            HMENU(8),
            GetModuleHandleW(None)?,
            None,
        );
        DRAG.with(|s| s.set(false));
        FONT_DPI.with(|d| d.set(0));
        layout(root);
        ShowWindow(root, SW_SHOW);
        SetForegroundWindow(root);
        let mut msg = MSG::default();
        let mut fullscreen = false;
        let mut restore = RECT::default();
        let mut result = Ok(());
        let mut refreshed = Instant::now() - Duration::from_secs(1);
        'running: loop {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    break 'running;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if !active() || GENERATION.load(Ordering::Acquire) != generation {
                let _ = DestroyWindow(root);
                break;
            }
            let desired = FULL.with(Cell::get);
            if desired != fullscreen {
                if desired {
                    let _ = GetWindowRect(root, &mut restore);
                    let monitor = MonitorFromWindow(root, MONITOR_DEFAULTTONEAREST);
                    let mut info = MONITORINFO {
                        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                        ..Default::default()
                    };
                    GetMonitorInfoW(monitor, &mut info);
                    SetWindowLongPtrW(root, GWL_STYLE, (WS_POPUP | WS_VISIBLE).0 as isize);
                    let r = info.rcMonitor;
                    let _ = SetWindowPos(
                        root,
                        None,
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_FRAMECHANGED | SWP_NOZORDER,
                    );
                } else {
                    SetWindowLongPtrW(
                        root,
                        GWL_STYLE,
                        (WS_OVERLAPPEDWINDOW | WS_VISIBLE).0 as isize,
                    );
                    let r = restore;
                    let _ = SetWindowPos(
                        root,
                        None,
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_FRAMECHANGED | SWP_NOZORDER,
                    );
                }
                fullscreen = desired;
                layout(root);
            }
            if let Some(p) = player() {
                if let Err(error) = p.UpdateVideo() {
                    if p.GetState().is_err() {
                        result = Err(error.into());
                        break;
                    }
                }
            }
            if refreshed.elapsed() >= Duration::from_millis(200) {
                refresh_controls(root);
                refreshed = Instant::now();
            }
            std::thread::sleep(Duration::from_millis(16));
        }
        PLAYER.with(|p| {
            if let Some(p) = p.borrow_mut().take() {
                let _ = p.Shutdown();
            }
        });
        if IsWindow(root).as_bool() {
            let _ = DestroyWindow(root);
        }
        ACTIVE_VIDEO_WINDOW.store(0, Ordering::SeqCst);
        ROOT.with(|s| s.set(HWND(0)));
        SURFACE.with(|s| s.set(HWND(0)));
        result
    }
}

pub(super) fn is_video_window_active() -> bool {
    ACTIVE_VIDEO_WINDOW.load(Ordering::SeqCst) != 0
}

pub(super) fn control_play_pause() {
    let hwnd = ACTIVE_VIDEO_WINDOW.load(Ordering::SeqCst);
    if hwnd != 0 {
        unsafe {
            let _ = PostMessageW(HWND(hwnd), WM_COMMAND, WPARAM(1), LPARAM(0));
        }
    }
}

pub(super) fn control_seek(target_secs: f32) {
    let hwnd = ACTIVE_VIDEO_WINDOW.load(Ordering::SeqCst);
    if hwnd != 0 && target_secs >= 0.0 {
        REQUESTED_SEEK_100NS.store((target_secs * 10_000_000.0) as i64, Ordering::SeqCst);
        unsafe {
            let _ = PostMessageW(HWND(hwnd), WM_APP_SEEK, WPARAM(0), LPARAM(0));
        }
    }
}

pub(super) fn control_set_volume(volume: f32) {
    let hwnd = ACTIVE_VIDEO_WINDOW.load(Ordering::SeqCst);
    if hwnd != 0 {
        REQUESTED_VOLUME.store((volume.clamp(0.0, 1.0) * 1000.0) as u32, Ordering::SeqCst);
        unsafe {
            let _ = PostMessageW(HWND(hwnd), WM_APP_SET_VOL, WPARAM(0), LPARAM(0));
        }
    }
}
