//! Native Windows notification-area menu. Its message window stays alive while
//! the application window is hidden; menu actions wake the root HWND directly.
use anyhow::Result;
use eframe::egui::{Context, ViewportCommand, ViewportId};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, Once,
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

#[link(name = "user32")]
extern "system" {
    fn ShowWindow(hwnd: *mut std::ffi::c_void, command: i32) -> i32;
    fn SetForegroundWindow(hwnd: *mut std::ffi::c_void) -> i32;
    fn GetForegroundWindow() -> *mut std::ffi::c_void;
    fn IsIconic(hwnd: *mut std::ffi::c_void) -> i32;
    fn IsWindowVisible(hwnd: *mut std::ffi::c_void) -> i32;
    fn PostMessageW(hwnd: *mut std::ffi::c_void, message: u32, wparam: usize, lparam: isize)
        -> i32;
}
#[derive(Clone)]
pub(crate) struct Control {
    ctx: Context,
    hwnd: isize,
    quitting: Arc<AtomicBool>,
    hidden: Arc<AtomicBool>,
    restored: Arc<AtomicBool>,
}
impl Control {
    fn command(&self, command: ViewportCommand) {
        self.ctx.send_viewport_cmd_to(ViewportId::ROOT, command);
        self.ctx.request_repaint_of(ViewportId::ROOT);
    }
    pub(crate) fn open(&self) {
        if self.quitting.load(Ordering::SeqCst) {
            return;
        }
        if self.hidden.swap(false, Ordering::SeqCst) {
            self.restored.store(true, Ordering::SeqCst);
        }
        // SAFETY: the HWND belongs to the live root viewport. These user32 calls
        // accept window handles from any thread and do not dereference Rust data.
        unsafe {
            let hwnd = self.hwnd as *mut std::ffi::c_void;
            ShowWindow(hwnd, if IsIconic(hwnd) != 0 { 9 } else { 5 });
            SetForegroundWindow(hwnd);
        }
        self.command(ViewportCommand::Visible(true));
        self.command(ViewportCommand::Minimized(false));
        self.command(ViewportCommand::Focus);
    }
    pub(crate) fn quit(&self) {
        self.quitting.store(true, Ordering::SeqCst);
        // Ensure a hidden window processes the final eframe close event. Showing
        // without activation avoids stealing focus from the user's foreground app.
        // SAFETY: the HWND belongs to the live root viewport. These user32 calls
        // accept window handles from any thread and do not dereference Rust data.
        unsafe {
            ShowWindow(self.hwnd as *mut _, 8);
        }
        self.command(ViewportCommand::Visible(true));
        self.command(ViewportCommand::Close);
        // SAFETY: the HWND belongs to the live root viewport. These user32 calls
        // accept window handles from any thread and do not dereference Rust data.
        unsafe {
            PostMessageW(self.hwnd as *mut _, 0x0010, 0, 0);
        }
    }
    pub(crate) fn hide_on_close(&self) {
        self.hidden.store(true, Ordering::SeqCst);
        self.command(ViewportCommand::CancelClose);
        self.command(ViewportCommand::Visible(false));
        // SAFETY: the HWND belongs to the live root viewport. These user32 calls
        // accept window handles from any thread and do not dereference Rust data.
        unsafe {
            ShowWindow(self.hwnd as *mut _, 0);
        }
    }
    pub(crate) fn quitting(&self) -> bool {
        self.quitting.load(Ordering::SeqCst)
    }
    pub(crate) fn visible(&self) -> bool {
        // SAFETY: the HWND belongs to the live root viewport. These user32 calls
        // accept window handles from any thread and do not dereference Rust data.
        unsafe { IsWindowVisible(self.hwnd as *mut _) != 0 }
    }
    pub(crate) fn close_window(&self) {
        self.command(ViewportCommand::Close);
        // SAFETY: the HWND belongs to the live root viewport. These user32 calls
        // accept window handles from any thread and do not dereference Rust data.
        unsafe {
            PostMessageW(self.hwnd as *mut _, 0x0010, 0, 0);
        }
    }
}
#[derive(Clone)]
struct Dispatch {
    open: MenuId,
    quit: MenuId,
    control: Control,
}
static DISPATCH: Mutex<Option<Dispatch>> = Mutex::new(None);
static HANDLER: Once = Once::new();

pub(crate) struct Tray {
    _icon: TrayIcon,
    control: Control,
}
impl Tray {
    pub(crate) fn new(ctx: Context, hwnd: isize) -> Result<Self> {
        let menu = Menu::new();
        let open = MenuItem::new("Mở Apps", true, None);
        let quit = MenuItem::new("Thoát Apps", true, None);
        menu.append_items(&[&open, &quit])?;
        let rgba = crate::limits::decode_image(include_bytes!("assets/app_icon.png"))?.to_rgba8();
        let (w, h) = rgba.dimensions();
        let normal_icon = Icon::from_rgba(rgba.clone().into_raw(), w, h)?;
        let mut marked = rgba;
        let radius = (w.min(h) / 5).max(1) as i64;
        let center = (w as i64 - radius - 1, radius + 1);
        for (x, y, pixel) in marked.enumerate_pixels_mut() {
            let distance = (x as i64 - center.0).pow(2) + (y as i64 - center.1).pow(2);
            if distance <= radius.pow(2) {
                *pixel = image::Rgba([220, 38, 38, 255]);
            }
        }
        let unread_icon = Icon::from_rgba(marked.into_raw(), w, h)?;
        let icon = normal_icon.clone();
        let tray = TrayIconBuilder::new()
            .with_tooltip("ctdozlite — Mở Apps / Thoát Apps")
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(true)
            .build()?;
        let timer_hwnd = tray.window_handle();
        // SAFETY: the tray HWND and callback live until Tray::drop cancels this owner-thread timer.
        if unsafe {
            SetTimer(
                timer_hwnd.cast(),
                NOTIFICATION_TIMER,
                40,
                Some(notification_tick),
            )
        } == 0
        {
            anyhow::bail!("Không thể khởi tạo thông báo nền ở khay hệ thống");
        }
        NOTIFICATION_ICON.with(|slot| {
            *slot.borrow_mut() = Some(NotificationIcon {
                icon: tray.clone(),
                normal: normal_icon,
                unread: unread_icon,
                shown: false,
                desktop: crate::desktop_notice_windows::Desktop::new(hwnd).ok(),
            })
        });
        let control = Control {
            ctx,
            hwnd,
            quitting: Arc::new(AtomicBool::new(false)),
            hidden: Arc::new(AtomicBool::new(false)),
            restored: Arc::new(AtomicBool::new(false)),
        };
        *DISPATCH
            .lock()
            .map_err(|_| anyhow::anyhow!("Tray dispatcher unavailable"))? = Some(Dispatch {
            open: open.id().clone(),
            quit: quit.id().clone(),
            control: control.clone(),
        });
        HANDLER.call_once(|| {
            tray_icon::TrayIconEvent::set_event_handler(Some(|event: tray_icon::TrayIconEvent| {
                if matches!(event, tray_icon::TrayIconEvent::DoubleClick { .. }) {
                    if let Some(dispatch) = DISPATCH.lock().ok().and_then(|state| state.clone()) {
                        dispatch.control.open();
                    }
                }
            }));
            MenuEvent::set_event_handler(Some(|event: MenuEvent| {
                let dispatch = DISPATCH.lock().ok().and_then(|state| state.clone());
                if let Some(dispatch) = dispatch {
                    if event.id == dispatch.open {
                        dispatch.control.open();
                    } else if event.id == dispatch.quit {
                        dispatch.control.quit();
                    }
                }
            }))
        });
        Ok(Self {
            _icon: tray,
            control,
        })
    }
    pub(crate) fn control(&self) -> Control {
        self.control.clone()
    }
}
impl Drop for Tray {
    fn drop(&mut self) {
        // SAFETY: this is the same owner thread and live tray HWND used to create the timer.
        unsafe {
            KillTimer(self._icon.window_handle().cast(), NOTIFICATION_TIMER);
        }
        NOTIFICATION_ICON.with(|slot| *slot.borrow_mut() = None);
        HAS_UNREAD.store(false, Ordering::Release);
        POPUP_PENDING.store(false, Ordering::Release);
        if let Ok(mut dispatch) = DISPATCH.lock() {
            if dispatch
                .as_ref()
                .is_some_and(|state| Arc::ptr_eq(&state.control.quitting, &self.control.quitting))
            {
                *dispatch = None;
            }
        }
    }
}

/// Exercise the same controls as the tray menu against the real application HWND.
/// The parent packaging script additionally requires the process to terminate.
pub(crate) fn smoke_lifecycle(control: Control) {
    std::thread::spawn(move || {
        let wait_visible = |expected: bool| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while control.visible() != expected {
                if std::time::Instant::now() >= deadline {
                    eprintln!("Tray smoke failed: expected visible={expected}");
                    std::process::exit(3);
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        };
        control.close_window();
        wait_visible(false);
        std::thread::sleep(std::time::Duration::from_millis(200));
        control.open();
        wait_visible(true);
        // Let eframe consume the show command before testing a second close.
        std::thread::sleep(std::time::Duration::from_millis(200));
        control.close_window();
        wait_visible(false);
        eprintln!("Tray smoke passed: X hides, Open restores, Exit requested while hidden");
        control.quit();
    });
}

/// Avoid retaining every history page in an unbounded GUI queue while hidden.
pub(crate) fn hidden() -> bool {
    DISPATCH
        .lock()
        .ok()
        .and_then(|d| d.as_ref().map(|d| d.control.hidden.load(Ordering::SeqCst)))
        .unwrap_or(false)
}

/// Consume the restore signal once; the UI reloads changes persisted while hidden.
pub(crate) fn take_restore_request() -> bool {
    DISPATCH
        .lock()
        .ok()
        .and_then(|d| {
            d.as_ref()
                .map(|d| d.control.restored.swap(false, Ordering::SeqCst))
        })
        .unwrap_or(false)
}

/// Wake the native window for an incoming call even when rendering is suspended.
// Not called since cbbb011 removed it from the incoming-call path; kept until the
// maintainer decides whether a hidden window should open for incoming calls again.
#[allow(dead_code)]
pub(crate) fn show_incoming_call() {
    let control = DISPATCH
        .lock()
        .ok()
        .and_then(|d| d.as_ref().map(|d| d.control.clone()));
    if let Some(control) = control {
        if control.hidden.load(Ordering::SeqCst) {
            control.open();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hidden_window_requests_one_reload_when_restored() {
        let control = Control {
            ctx: Context::default(),
            hwnd: 0,
            quitting: Arc::new(AtomicBool::new(false)),
            hidden: Arc::new(AtomicBool::new(false)),
            restored: Arc::new(AtomicBool::new(false)),
        };
        *DISPATCH.lock().unwrap() = Some(Dispatch {
            open: MenuId::new("fixture-open"),
            quit: MenuId::new("fixture-quit"),
            control: control.clone(),
        });
        assert!(!hidden());
        control.hide_on_close();
        assert!(hidden());
        assert!(!take_restore_request());
        control.open();
        assert!(!hidden());
        assert!(take_restore_request());
        assert!(!take_restore_request());
        *DISPATCH.lock().unwrap() = None;
    }
}

/// Suppress GPU preview work when the window is hidden or minimized.
pub(crate) fn previews_visible() -> bool {
    let control = DISPATCH
        .lock()
        .ok()
        .and_then(|d| d.as_ref().map(|d| d.control.clone()));
    control.is_none_or(|c| {
        !c.hidden.load(Ordering::SeqCst) && c.visible() && {
            // SAFETY: user32 validates the live root HWND and permits cross-thread queries.
            unsafe { IsIconic(c.hwnd as *mut _) == 0 }
        }
    })
}

// Native timers run on the tray message loop even when eframe stops painting a hidden viewport.
// TrayIcon is Rc-based and must only be touched by that owner thread.
struct NotificationIcon {
    icon: TrayIcon,
    normal: Icon,
    unread: Icon,
    shown: bool,
    desktop: Option<crate::desktop_notice_windows::Desktop>,
}
thread_local! {
    static NOTIFICATION_ICON: std::cell::RefCell<Option<NotificationIcon>> = const { std::cell::RefCell::new(None) };
}
static HAS_UNREAD: AtomicBool = AtomicBool::new(false);
static NOTIFICATIONS_ENABLED: AtomicBool = AtomicBool::new(true);
static NOTIFICATION_SOUND: AtomicBool = AtomicBool::new(false);
static POPUP_PENDING: AtomicBool = AtomicBool::new(false);
const NOTIFICATION_TIMER: usize = 0x4354444f;
#[link(name = "user32")]
extern "system" {
    fn SetTimer(
        hwnd: *mut std::ffi::c_void,
        id: usize,
        interval: u32,
        callback: Option<unsafe extern "system" fn(*mut std::ffi::c_void, u32, usize, u32)>,
    ) -> usize;
    fn KillTimer(hwnd: *mut std::ffi::c_void, id: usize) -> i32;

}
unsafe extern "system" fn notification_tick(_: *mut std::ffi::c_void, _: u32, _: usize, _: u32) {
    NOTIFICATION_ICON.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return;
        };
        let Some(icon) = slot.as_mut() else {
            return;
        };
        if let Some(desktop) = icon.desktop.as_mut() {
            desktop.tick(!foreground());
        }
        let unread = crate::desktop_notice::snapshot().count > 0;
        if unread != icon.shown {
            let image = if unread { &icon.unread } else { &icon.normal };
            if icon.icon.set_icon(Some(image.clone())).is_ok() {
                icon.shown = unread;
                let tip = if unread {
                    "ctdozlite — Có tin nhắn chưa đọc"
                } else {
                    "ctdozlite — Mở Apps / Thoát Apps"
                };
                if icon.icon.set_tooltip(Some(tip)).is_err() {
                    crate::logger::log_error(
                        "tray attention",
                        "Cannot update notification tooltip",
                    );
                }
            }
        }
    });
}
pub(crate) fn configure_notifications(enabled: bool, sound: bool) {
    if !enabled {
        POPUP_PENDING.store(false, Ordering::Release);
    }
    NOTIFICATIONS_ENABLED.store(enabled, Ordering::Relaxed);
    NOTIFICATION_SOUND.store(sound, Ordering::Relaxed);
}
pub(crate) fn set_unread(unread: bool) {
    HAS_UNREAD.store(unread, Ordering::Release);
}

/// A popup click owns the exact notification snapshot, independent of newer arrivals.
pub(crate) fn open_notice(notice: crate::desktop_notice::Notice) {
    crate::desktop_notice::activate(notice);
    if let Some(control) = DISPATCH
        .lock()
        .ok()
        .and_then(|s| s.as_ref().map(|d| d.control.clone()))
    {
        control.open();
    }
}

/// Visible does not imply foreground: another application may cover the root window.
pub(crate) fn foreground() -> bool {
    let control = DISPATCH
        .lock()
        .ok()
        .and_then(|d| d.as_ref().map(|d| d.control.clone()));
    control.is_some_and(|c| {
        // SAFETY: user32 returns a handle; comparison does not dereference it.
        !c.hidden.load(Ordering::SeqCst) && unsafe { GetForegroundWindow() as isize == c.hwnd }
    })
}
