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
        let icon = Icon::from_rgba(rgba.into_raw(), w, h)?;
        let tray = TrayIconBuilder::new()
            .with_tooltip("ctdozlite — Mở Apps / Thoát Apps")
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(true)
            .build()?;
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
        if let Ok(mut dispatch) = DISPATCH.lock() {
            if dispatch.as_ref().map_or(false, |state| {
                Arc::ptr_eq(&state.control.quitting, &self.control.quitting)
            }) {
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
