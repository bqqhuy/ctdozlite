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
