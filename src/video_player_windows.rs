//! MFPlay renders and synchronizes audio/video in a native, worker-owned window.
use anyhow::{Context, Result};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Once,
    },
    time::Duration,
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
thread_local! {
    static PLAYER:RefCell<Option<IMFPMediaPlayer>>=const{RefCell::new(None)};
    static ROOT:Cell<HWND>=const{Cell::new(HWND(0))};
    static SURFACE:Cell<HWND>=const{Cell::new(HWND(0))};
    static FULL:Cell<bool>=const{Cell::new(false)};
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
unsafe fn layout(hwnd: HWND) {
    let mut r = RECT::default();
    let _ = GetClientRect(hwnd, &mut r);
    let video = SURFACE.with(Cell::get);
    if video.0 != 0 {
        let _ = MoveWindow(video, 0, 0, r.right, (r.bottom - 56).max(1), true);
    }
    let widths = [130, 90, 90, 85, 85, 135];
    let mut x = 8;
    for (index, width) in widths.into_iter().enumerate() {
        let button = GetDlgItem(hwnd, index as i32 + 1);
        if button.0 != 0 {
            let _ = MoveWindow(button, x, (r.bottom - 46).max(0), width, 32, true);
        }
        x += width + 6;
    }
    if let Some(p) = player() {
        let _ = p.UpdateVideo();
    }
}
unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
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
            }
            EndPaint(hwnd, &ps);
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
                _ => {}
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            match wp.0 {
                0x1b => {
                    let _ = DestroyWindow(ROOT.with(Cell::get));
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
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            if hwnd == ROOT.with(Cell::get) {
                PostQuitMessage(0);
            }
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
            "Toàn màn hình",
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
        layout(root);
        ShowWindow(root, SW_SHOW);
        SetForegroundWindow(root);
        let mut msg = MSG::default();
        let mut fullscreen = false;
        let mut restore = RECT::default();
        let mut result = Ok(());
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
        ROOT.with(|s| s.set(HWND(0)));
        SURFACE.with(|s| s.set(HWND(0)));
        result
    }
}
