//! Phone-approved history transfer over the existing authenticated socket.
mod assets;
pub mod codec;
mod import;
mod pipeline;
mod plan;
mod policy;
mod transfer;
mod wire;
pub(crate) use transfer::download;

pub use import::AtomicRestoreSession;
pub(crate) use transfer::{clean_after_connect, request_wake};

pub(crate) mod legacy;
