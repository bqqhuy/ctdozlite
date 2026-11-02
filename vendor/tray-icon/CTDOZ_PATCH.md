# Local tray-icon 0.21.3 patch

Upstream: https://github.com/tauri-apps/tray-icon (crate 0.21.3).
Original Apache-2.0 and MIT licenses are retained.

- Add Windows-only `TrayIcon::show_notification`, implemented with
  `Shell_NotifyIconW(NIM_MODIFY, NIF_INFO)` and the instance native ID.
- Bound UTF-16 title/body buffers and support NIIF_NOSOUND.
- Convert NIN_BALLOONUSERCLICK into the existing DoubleClick event so the app
  can open its window without an additional unmanaged callback.

No endpoint, icon identity, dependency version or non-Windows behavior changes.
