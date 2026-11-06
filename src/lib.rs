/// User-visible fork release identifier, independent of the numeric Windows file version.
pub const PRODUCT_VERSION: &str = "v0.008-arm";

pub mod call;
pub mod chat;
pub mod clock;
pub mod crypto;
pub mod endpoints;
pub mod gui;
pub mod limits;
pub mod logger;
pub mod login;
pub mod media;
pub mod message_view;
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

pub mod mentions;
pub mod outgoing;

pub mod upload;

pub mod tools;

/// Account-wide conversation mute state shared with official Zalo clients.
pub mod mute_sync;

/// Display-only styling for Zalo rich-text messages.
pub mod rich_text;

pub mod clipboard_image;
pub mod crash_report;
/// Optional outbound proxy for Zalo HTTP and WebSocket traffic.
pub mod proxy;

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

/// Optional HD copy of a photo before upload.
pub mod photo_quality;
/// Typing indicators and delivery/read receipts.
pub mod presence;
/// Queue cursors kept across app starts.
pub mod queue_cursors;
/// Region screenshot for the composer (Windows snipping overlay).
pub mod screen_capture;
/// Correlates async file uploads with their processed-URL notifications.
pub mod upload_events;
/// Local video size, duration and poster for video messages.
pub mod video_meta;

mod attachment_cache;

#[cfg(windows)]
mod drop_cursor_windows;

pub mod search;

pub mod server_pins;

pub mod board;

pub mod i18n;

pub mod group_state;

pub mod api_response;
pub mod archive;
pub mod avatar;
pub mod contact_actions;
pub mod group_entry;
pub mod groups;
pub mod inbox;
pub mod labels;
pub mod privacy;
pub mod profile;

pub mod desktop_backup;
mod desktop_backup_format;

pub mod message_lifecycle;

pub mod conversation_policy;

pub mod group_content;

mod reaction_pipeline;

mod group_metadata;
mod history_open;

pub mod avatar_history;

pub mod last_activity;

pub mod hidden_chat;

pub mod account_controls;
pub mod account_metadata;
