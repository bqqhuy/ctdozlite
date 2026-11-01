pub mod call;
pub mod chat;
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
