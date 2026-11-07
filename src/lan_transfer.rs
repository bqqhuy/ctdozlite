//! Local Area Network (LAN) File Transfer & Peer Discovery (Zalo Parity C03).
//!
//! Provides signaling data structures and discovery protocol helpers for high-speed
//! direct device-to-device transfers within the same local Wi-Fi/LAN network
//! without going through cloud servers (`/api/lan/discover`, `/api/lan/transfer`).

use serde::{Deserialize, Serialize};

/// A discovered peer device on the local network.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LanPeer {
    pub peer_id: String,
    pub device_name: String,
    pub ip_address: String,
    pub port: u16,
    pub last_seen_ts: i64,
}

/// State of an ongoing LAN transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LanTransferState {
    Idle,
    Connecting,
    Transferring { bytes_transferred: u64, total_bytes: u64 },
    Finished,
    Failed(String),
    Cancelled,
}

impl LanTransferState {
    pub fn progress_ratio(&self) -> f32 {
        match self {
            Self::Transferring { bytes_transferred, total_bytes } => {
                if *total_bytes > 0 {
                    (*bytes_transferred as f32 / *total_bytes as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            }
            Self::Finished => 1.0,
            _ => 0.0,
        }
    }
}

/// A session representing a local transfer between peers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LanTransferSession {
    pub session_id: String,
    pub file_name: String,
    pub file_size: u64,
    pub target_peer: LanPeer,
    pub state: LanTransferState,
}

/// Parse a LAN UDP discovery packet.
/// Format: `ZALO_LAN:v1:<peer_id>:<device_name>:<port>`
pub fn parse_lan_discovery_packet(payload: &str, sender_ip: &str, now_ts: i64) -> Option<LanPeer> {
    let parts: Vec<&str> = payload.trim().split(':').collect();
    if parts.len() < 5 || parts[0] != "ZALO_LAN" || parts[1] != "v1" {
        return None;
    }

    let peer_id = parts[2].to_string();
    let device_name = parts[3].to_string();
    let port = parts[4].parse::<u16>().ok()?;

    Some(LanPeer {
        peer_id,
        device_name,
        ip_address: sender_ip.to_string(),
        port,
        last_seen_ts: now_ts,
    })
}

/// Format a LAN UDP broadcast announcement.
pub fn format_lan_discovery_packet(peer_id: &str, device_name: &str, port: u16) -> String {
    format!("ZALO_LAN:v1:{peer_id}:{device_name}:{port}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lan_peer_discovery() {
        let packet = "ZALO_LAN:v1:peer_win11_surface:Surface Pro ARM64:8998";
        let peer = parse_lan_discovery_packet(packet, "192.168.1.105", 1795000000).expect("Must parse peer");
        assert_eq!(peer.peer_id, "peer_win11_surface");
        assert_eq!(peer.device_name, "Surface Pro ARM64");
        assert_eq!(peer.ip_address, "192.168.1.105");
        assert_eq!(peer.port, 8998);
        assert_eq!(peer.last_seen_ts, 1795000000);

        let formatted = format_lan_discovery_packet("peer_win11_surface", "Surface Pro ARM64", 8998);
        assert_eq!(formatted, packet);
    }

    #[test]
    fn test_lan_transfer_state() {
        let state = LanTransferState::Transferring {
            bytes_transferred: 50_000,
            total_bytes: 100_000,
        };
        assert_eq!(state.progress_ratio(), 0.5);

        let done = LanTransferState::Finished;
        assert_eq!(done.progress_ratio(), 1.0);

        let idle = LanTransferState::Idle;
        assert_eq!(idle.progress_ratio(), 0.0);
    }
}
