//! Client coordinates for OLE file drops whose positions winit 0.30 discards.
use windows::Win32::{
    Foundation::{HWND, POINT},
    Graphics::Gdi::ScreenToClient,
    UI::WindowsAndMessaging::GetCursorPos,
};

/// Sample the live cursor in physical client pixels of eframe's borrowed root window.
/// An unavailable/destroyed window fails closed; callers must not use a stale pointer.
pub(crate) fn position(hwnd: isize) -> Option<(i32, i32)> {
    let mut point = POINT::default();
    // Win32 validates the borrowed HWND; POINT storage lives across both synchronous calls.
    if unsafe {
        GetCursorPos(&mut point).is_ok() && ScreenToClient(HWND(hwnd), &mut point).as_bool()
    } {
        Some((point.x, point.y))
    } else {
        None
    }
}
