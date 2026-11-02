//! Zalo client in Rust — complete TUI & GUI.
//!
//! Protocol reimplemented from Zalo PC 26.10.11.

#![allow(dead_code)]

use zalo_rust::{chat, gui, logger, login, platform, qr, session, storage, store, tui, ui, video};

#[cfg(all(not(debug_assertions), not(all(windows, target_arch = "aarch64"))))]
compile_error!("Production builds of this fork require Windows ARM64 (aarch64-pc-windows-msvc)");

use anyhow::Result;
use session::Session;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("ctdozlite v{} (Windows ARM64)", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--licenses") {
        println!("{}", include_str!("../THIRD_PARTY_NOTICES.txt"));
        return Ok(());
    }
    let smoke = args.iter().any(|a| a == "--ui-smoke-test");
    anyhow::ensure!(
        !args.iter().any(|a| a == "--tray-smoke-test") || smoke,
        "--tray-smoke-test requires --ui-smoke-test"
    );
    if smoke && std::env::var_os("CTDOZ_DATA_DIR").is_none() {
        anyhow::bail!("--ui-smoke-test requires an isolated CTDOZ_DATA_DIR");
    }
    let profile = storage::profile_from_args(&args)?;
    storage::init_profile(args.iter().any(|a| a == "--portable"), profile.as_deref())?;
    let proxy = zalo_rust::proxy::from_args(&args, std::env::var("CTDOZ_PROXY").ok())?;
    if let Some(config) = &proxy {
        println!("Proxy: {}", config.redacted());
    }
    zalo_rust::proxy::init(proxy)?;
    zalo_rust::crash_report::install();
    let _instance = if args.iter().any(|a| a == "--doctor") {
        None
    } else {
        Some(storage::lock_instance()?)
    };
    let dev_mode = args
        .iter()
        .any(|a| a == "--dev" || a == "-v" || a == "--verbose");
    let dump = args.iter().any(|a| a == "-d" || a == "--dump");
    let dump_raw = args.iter().any(|a| a == "--dump-raw");
    let force_tui = args.iter().any(|a| a == "--tui");
    let force_gui = args.iter().any(|a| a == "--gui");
    let force_qr = args.iter().any(|a| a == "--qr");
    let force_logout = args.iter().any(|a| a == "--logout");

    if force_logout {
        storage::remove_session()?;
        println!("✓ Đã đăng xuất. Lịch sử được giữ riêng cho tài khoản đã đăng nhập.");
        if !force_gui && !force_tui && !force_qr {
            return Ok(());
        }
    }

    logger::init(dev_mode);

    if dev_mode {
        println!(
            "🔧 [DEV MODE ACTIVATED] – Quét log, chẩn đoán giao thức và trace chi tiết được bật."
        );
    } else {
        println!("🔒 [PRODUCTION MODE] – Toàn bộ nhật ký và dev inspector được ẩn.");
    }

    if args.iter().any(|a| a == "--doctor") {
        println!("Architecture: {}", std::env::consts::ARCH);
        println!("Data directory: {}", storage::root().display());
        if let Some(name) = &profile {
            println!("Profile: {name}");
        }
        match video::readiness() {
            Ok(path) => println!("Video ready: {path}"),
            Err(e) => println!("Video unavailable: {e}"),
        }
        return Ok(());
    }
    let mut s = Session::new(dev_mode, dump, dump_raw)?;

    // Attempt to restore persistent session from disk
    let session_restored = if !force_logout && !smoke {
        if s.load_from_file("session.json").is_ok() {
            if let Some(li) = &s.login {
                let name = if li.display_name.is_empty() {
                    "Zalo User"
                } else {
                    &li.display_name
                };
                println!(
                    "✓ Đã khôi phục phiên từ session.json: {name} (UIN: {})",
                    li.uin
                );
                true
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    // Determine launch mode:
    // If `--gui` is specified (or default when not `--tui` / `--qr` / `--phone`):
    // Open the native GUI window IMMEDIATELY without blocking on network requests!
    // Background worker in `gui.rs` handles `get_server_info`, `get_friends`, and `poll_new_messages`.
    if force_gui || (!force_tui && !force_qr && !args.iter().any(|a| a.starts_with("--phone"))) {
        println!("🚀 Đang khởi chạy giao diện đồ họa Zalo GUI (egui)…");
        #[cfg(target_os = "windows")]
        if !dev_mode {
            #[link(name = "kernel32")]
            extern "system" {
                fn GetConsoleWindow() -> *mut std::ffi::c_void;
            }
            #[link(name = "user32")]
            extern "system" {
                fn ShowWindow(hwnd: *mut std::ffi::c_void, n_cmd_show: i32) -> i32;
            }
            unsafe {
                let hwnd = GetConsoleWindow();
                if !hwnd.is_null() {
                    ShowWindow(hwnd, 0); // SW_HIDE
                }
            }
        }
        let mut viewport = eframe::egui::ViewportBuilder::default()
            .with_decorations(true)
            .with_inner_size([1020.0, 700.0])
            .with_min_inner_size([480.0, 360.0])
            .with_title("Zalo");
        if smoke {
            let size = args
                .iter()
                .position(|a| a == "--ui-smoke-size")
                .and_then(|i| args.get(i + 1))
                .map(|s| {
                    s.split('x')
                        .filter_map(|n| n.parse::<f32>().ok())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| vec![480.0, 360.0]);
            anyhow::ensure!(
                size.len() == 2
                    && size
                        .iter()
                        .all(|n| n.is_finite() && *n >= 360.0 && *n <= 1920.0),
                "Invalid smoke size"
            );
            viewport = viewport.with_inner_size([size[0], size[1]]);
        }
        if let Some(icon) = gui::load_app_icon() {
            viewport = viewport.with_icon(std::sync::Arc::new(icon));
        }
        let mut initial_threads = chat::load_threads_cache(&s);
        store::load_history_into_threads(
            s.history_path(),
            &mut initial_threads,
            store::MessageStore::PRELOAD_MSGS,
        );

        #[cfg(all(windows, target_arch = "aarch64"))]
        if dev_mode {
            eprintln!(
                "Compiled graphics backends: {:?}",
                eframe::wgpu::Instance::enabled_backend_features()
            );
        }
        let renderers = gui_renderer_order();
        let mut last_err = None;
        for (i, renderer) in renderers.iter().enumerate() {
            if dev_mode {
                println!("🎨 Renderer: {renderer:?}");
            }
            let native_options = eframe::NativeOptions {
                viewport: viewport.clone(),
                renderer: *renderer,
                #[cfg(all(windows, target_arch = "aarch64"))]
                wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
                    supported_backends: eframe::wgpu::Backends::DX12,
                    ..Default::default()
                },
                ..Default::default()
            };
            let app_session = s.clone();
            let threads = initial_threads.clone();
            match eframe::run_native(
                "Zalo - Rust Native Client",
                native_options,
                Box::new(move |cc| {
                    #[cfg(all(windows, target_arch = "aarch64"))]
                    if let Some(state) = &cc.wgpu_render_state {
                        let info = state.adapter.get_info();
                        eprintln!("Graphics: {} ({:?})", info.name, info.backend);
                    }
                    Ok(Box::new(gui::ZaloGuiApp::new(
                        cc,
                        app_session,
                        dev_mode,
                        threads,
                    )))
                }),
            ) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    // Only fall through when the graphics backend itself failed to
                    // initialise; that happens before the window is ever shown.
                    eprintln!("⚠️ Renderer {renderer:?} lỗi: {e}");
                    last_err = Some(e);
                    if i + 1 < renderers.len() {
                        eprintln!("↪ Thử renderer dự phòng…");
                    }
                }
            }
        }
        let message = format!(
            "Không thể khởi chạy GUI: {}",
            last_err.map(|e| e.to_string()).unwrap_or_default()
        );
        if !smoke {
            platform::startup_error(&message);
        }
        return Err(anyhow::anyhow!(message));
    }

    // 1. Initial server info check for TUI mode
    if let Ok(resp) = login::get_server_info(&s).await {
        if let Some(d) = resp.get("data") {
            s.apply_server_info(d);
        }
        if dev_mode {
            println!(
                "✓ getServerInfo: chat {} • convers {}",
                s.domains.chat, s.domains.conversation
            );
        }
    }

    // Otherwise, run in TUI mode:
    println!("🖥️ Khởi chạy chế độ dòng lệnh TUI…");

    // Validate session on server to avoid getting stuck in an expired session
    let session_restored = if session_restored {
        if s.validate_session().await != session::SessionStatus::Expired {
            true
        } else {
            println!(
                "⚠️ Phiên đăng nhập trong session.json đã hết hạn. Đang chuyển sang đăng nhập mới…"
            );
            storage::remove_session()?;
            s.logout();
            false
        }
    } else {
        false
    };

    // Login via QR or credentials if not already restored
    if force_qr || !session_restored {
        if force_qr {
            println!("… Đang tạo mã QR");
            let _ = qr::login_via_qr(&mut s, "vi").await?;
        } else {
            let arg_val = |name: &str| -> Option<String> {
                args.iter()
                    .position(|a| a == name)
                    .and_then(|i| args.get(i + 1))
                    .cloned()
            };

            let phone = match arg_val("--phone") {
                Some(v) => v,
                None => ui::prompt("Số điện thoại: ")?,
            };
            let password = match arg_val("--pass") {
                Some(v) => v,
                None => ui::prompt_password("Mật khẩu: ")?,
            };
            let iso = match arg_val("--iso") {
                Some(v) => v,
                None => ui::prompt_default("Mã quốc gia", "VN")?,
            };

            if !phone.trim().is_empty() && !password.is_empty() {
                println!("… Đang đăng nhập mật khẩu");
                let v1 = args.iter().any(|a| a == "--v1");
                let _ = if v1 {
                    login::login_by_password_v1(&mut s, phone.trim(), &password).await?
                } else {
                    login::login_by_password(&mut s, phone.trim(), &password, iso.trim()).await?
                };
            }
        }
    }

    if s.login.is_some() {
        if let Err(e) = s.save_to_file("session.json") {
            eprintln!("Không thể lưu phiên: {e}");
        }
    }

    // Fetch initial threads and hydrate with local SQLite history
    let mut initial_threads = chat::load_threads_cache(&s);
    store::load_history_into_threads(
        s.history_path(),
        &mut initial_threads,
        store::MessageStore::PRELOAD_MSGS,
    );
    if s.login.is_some() {
        match chat::get_friends(&s).await {
            Ok(list) => {
                for ft in list {
                    if let Some(ex) = initial_threads.iter_mut().find(|t| t.id == ft.id) {
                        if !ft.name.is_empty() {
                            ex.name = ft.name;
                        }
                        ex.is_group = ex.is_group || ft.is_group;
                    } else {
                        initial_threads.push(ft);
                    }
                }
                let mut poll_state = chat::PollState::default();
                if let Err(e) =
                    chat::poll_new_messages(&s, &mut poll_state, &mut initial_threads).await
                {
                    crate::logger::log_error("main::poll_new_messages", &e.to_string());
                    eprintln!("! Lỗi tải tin nhắn gần nhất: {e}");
                }
            }
            Err(e) => {
                crate::logger::log_error("main::get_friends", &e.to_string());
                eprintln!("! Lỗi tải danh sách bạn bè: {e}");
            }
        }
    }

    let tui_app = tui::TuiApp::new(&mut s, initial_threads);
    tui::run_tui(tui_app).await?;

    Ok(())
}

/// Order in which GUI graphics backends are tried.
///
/// * Windows on ARM64: Direct3D 12 via wgpu first (always present), then OpenGL
///   (only works when the vendor driver or the "OpenGL Compatibility Pack" is installed).
/// * Everywhere else: OpenGL (`glow`), unchanged from upstream.
///
/// `CTDOZ_RENDERER=glow|wgpu` overrides the order for troubleshooting.
fn gui_renderer_order() -> Vec<eframe::Renderer> {
    #[allow(unused_mut)]
    let mut order = vec![eframe::Renderer::Glow];
    #[cfg(all(windows, target_arch = "aarch64"))]
    {
        order = vec![eframe::Renderer::Wgpu, eframe::Renderer::Glow];
    }
    match std::env::var("CTDOZ_RENDERER").ok().as_deref() {
        Some("glow") | Some("gl") | Some("opengl") => order = vec![eframe::Renderer::Glow],
        #[cfg(all(windows, target_arch = "aarch64"))]
        Some("wgpu") | Some("dx12") => order = vec![eframe::Renderer::Wgpu],
        _ => {}
    }
    order
}
