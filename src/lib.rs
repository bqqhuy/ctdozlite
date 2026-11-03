pub mod call;
pub mod chat;
pub mod message_view;
pub mod clock;
pub mod crypto;
pub mod endpoints;
pub mod gui;
pub mod limits;
pub mod logger;
pub mod login;
pub mod media;
pub mod param_cipher;
pub mod qr;
pub mod random;
pub mod session;
pub mod socket;
pub mod storage;
pub mod store;
pub mod tui;
pub mod ui;
pub mod video;
pub mod voip_log;

#[cfg(windows)]
mod windows_video;

mod history_sync;

pub mod platform;

#[cfg(windows)]
mod tray;

pub mod message_model;
pub mod reactions;
pub mod unread;

#[cfg(any(windows, test))]
mod video_frame;

/// Bounded media resources and admission control.
pub mod media_resources;
/// Release download validation and transactional file replacement.
pub mod updater;

mod video_preview;

mod diagnostics;

/// Phone-approved encrypted history transfer.
pub mod phone_sync;

/// Exact lookup of quoted messages already loaded in the active conversation.
pub mod message_navigation;

mod live_updates;

pub mod outgoing;

pub mod upload;

pub mod tools;

/// Account-wide conversation mute state shared with official Zalo clients.
pub mod mute_sync;

/// Display-only styling for Zalo rich-text messages.
pub mod rich_text;

/// Optional outbound proxy for Zalo HTTP and WebSocket traffic.
pub mod proxy;
pub mod clipboard_image;
pub mod crash_report;

pub mod media_download;

pub mod video_player;

pub mod call_lifecycle;

pub mod desktop_notice;

#[cfg(windows)]
mod desktop_notice_windows;

pub mod notification_sound;

mod bundled_stickers;

/// Media recovery classifications and finite retry state.
pub mod media_recovery;

/// Official FILE service media URL renewal.
pub mod media_renew;

/// Typed, bounded diagnostics available without development logging.
mod transfer_diagnostics;

mod queue_catchup;
