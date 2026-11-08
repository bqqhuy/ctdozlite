# Bridge Table Reference — Zalo PC Channel Architecture

**Date:** 2026-11-08  
**Purpose:** Document Zalo PC WebSocket bridge table channels and map to ZaloLite implementation  
**Status:** v0.015.2 reference (WP15-5)

---

## Overview

Zalo PC communicates via WebSocket using a **bridge table** of ~450 channels. Each channel:
- **Type:** REQUEST, NOTIFY, SUBSCRIBE, or RAW_SEND
- **Direction:** Request/Response (client ↔ server) or one-way push
- **Format:** Binary with `zconnect-` prefix namespace

---

## Channel Categories & ZaloLite Mapping

### 1. Connection & Transport (`zconnect-*`)
| Channel | Type | Purpose | ZaloLite Location | Status |
|---------|------|---------|-------------------|--------|
| `zconnect-config` | REQUEST | Initial handshake config | `src/socket.rs:init()` | ✅ Implemented |
| `zconnect-heartbeat` | NOTIFY | Keep-alive ping | `src/socket.rs:heartbeat` | ✅ Implemented |
| `zconnect-epoch` | NOTIFY | Session epoch change (reconnect signal) | `src/socket.rs:ready_watch` | ✅ Implemented |

**Status:** Transport layer complete. WebSocket + reconnect backoff in v0.015.1.

---

### 2. Messaging (`zmessage-*`)
| Channel | Type | Purpose | ZaloLite Location | Status |
|---------|------|---------|-------------------|--------|
| `zmessage-receive-sms` | NOTIFY | Incoming text messages | `src/chat.rs:receive_new_messages()` | ✅ Implemented |
| `zmessage-receive-group-sms` | NOTIFY | Group messages | `src/chat.rs:receive_new_messages()` | ✅ Implemented |
| `zmessage-send-sms` | REQUEST | Send text (plain) | `src/chat.rs:send_message_with_client_id()` | ✅ Implemented |
| `zmessage-send-group-sms` | REQUEST | Send group message | `src/chat.rs:send_group_message_with_client_id()` | ✅ Implemented |
| `zmessage-receipt` | NOTIFY | Delivery/read receipts | `src/presence.rs` (partial) | 🔄 Partial |
| `zmessage-typing-status` | NOTIFY | Typing indicators | `src/presence.rs` (partial) | 🔄 Partial |
| `zmessage-undo-message` | REQUEST | Recall message | `src/chat.rs:undo_message()` | ✅ Implemented |

**Status:** Core messaging done. Receipts/typing partial (not blocking messages).

---

### 3. E2EE Messaging (`zmessage-e2ee-*`) — **WP16 (v0.016)**
| Channel | Type | Purpose | ZaloLite Location | Status |
|---------|------|---------|-------------------|--------|
| `zmessage-e2ee-sms` | REQUEST | Send E2EE text | `src/chat.rs:send_e2ee_message()` (WIP) | 🔄 In Progress |
| `zmessage-e2ee-receive-sms` | NOTIFY | Receive E2EE text | `src/chat.rs:try_decrypt_e2ee_content()` (WIP) | 🔄 In Progress |
| `zmessage-e2ee-receipt` | NOTIFY | E2EE receipt | Planned v0.016 | ❌ Not Started |

**Status:** v0.015.2 added UI indicators (lock badge). Full encryption core in v0.016.

---

### 4. File Transfer (`zefile-*`)
| Channel | Type | Purpose | ZaloLite Location | Status |
|---------|------|---------|-------------------|--------|
| `zefile-upload-start` | REQUEST | Initiate chunked upload | `src/upload.rs:ResumableUploadState` | ✅ Implemented |
| `zefile-upload-chunk` | REQUEST | Send file chunk | `src/upload.rs` | ✅ Implemented |
| `zefile-upload-complete` | REQUEST | Finalize upload | `src/upload.rs` | ✅ Implemented |
| `zefile-download-url` | REQUEST | Get media URL | `src/media_download.rs` | ✅ Implemented |
| `zefile-range-request` | REQUEST | HTTP Range: bytes=X-Y | `src/media_download.rs` (RFC 7233) | ✅ v0.015 |

**Status:** Resumable upload state in v0.015.1. Range-request streaming in v0.015-arm.

---

### 5. Presence & Status (`zpresence-*`)
| Channel | Type | Purpose | ZaloLite Location | Status |
|---------|------|---------|-------------------|--------|
| `zpresence-online-status` | NOTIFY | Online/offline state | `src/presence.rs` | 🔄 Partial |
| `zpresence-typing` | NOTIFY | User typing | `src/presence.rs` | 🔄 Partial |
| `zpresence-last-seen` | NOTIFY | Last active timestamp | `src/presence.rs` | 🔄 Partial |

**Status:** Basic online detection works. Full presence features deferred.

---

### 6. Call & VoIP (`zcall-*`)
| Channel | Type | Purpose | ZaloLite Location | Status |
|---------|------|---------|-------------------|--------|
| `zcall-initiate` | REQUEST | Start voice/video call | `src/voip.rs` | ✅ Implemented |
| `zcall-ringing` | NOTIFY | Incoming call ring | `src/voip.rs` | ✅ Implemented |
| `zcall-accept` | REQUEST | Answer call | `src/voip.rs` | ✅ Implemented |
| `zcall-hangup` | REQUEST | End call | `src/voip.rs` | ✅ Implemented |
| `zcall-audio-stats` | NOTIFY | Codec/bitrate info | `src/voice.rs` (WP14) | ✅ v0.014 |

**Status:** VoIP core complete. AudioProcessor (AEC/NS/AGC) in v0.015.1.

---

### 7. Encryption & Keys (`zauth-e2ee-*`) — **WP16 (v0.016)**
| Channel | Type | Purpose | ZaloLite Location | Status |
|---------|------|---------|-------------------|--------|
| `zauth-e2ee-prekey-upload` | REQUEST | Register PreKey bundle | `src/crypto/ratchet.rs` (planned) | ❌ v0.016 |
| `zauth-e2ee-prekey-query` | REQUEST | Download peer PreKey | `src/crypto/ratchet.rs` (planned) | ❌ v0.016 |
| `zauth-e2ee-identity-verify` | REQUEST | Safety number verification | `src/crypto/safety.rs` (planned) | ❌ v0.016 |

**Status:** Crypto foundation laid (v0.015.2 E2EE bonus). PreKey registration in v0.016.

---

### 8. Group Management (`zgroup-*`)
| Channel | Type | Purpose | ZaloLite Location | Status |
|---------|------|---------|-------------------|--------|
| `zgroup-create` | REQUEST | Create group | `src/chat.rs` | ✅ Implemented |
| `zgroup-members-update` | NOTIFY | Member list change | `src/group_state.rs` | ✅ Implemented |
| `zgroup-name-update` | NOTIFY | Group name changed | `src/group_state.rs` | ✅ Implemented |

**Status:** Group management stable since v0.012+.

---

### 9. Cloud & Sync (`zcloud-*`)
| Channel | Type | Purpose | ZaloLite Location | Status |
|---------|------|---------|-------------------|--------|
| `zcloud-history-sync` | REQUEST | Fetch server history | `src/phone_sync.rs` | ✅ Implemented |
| `zcloud-media-index` | REQUEST | List cloud files | `src/zcloud.rs` | ✅ Implemented |

**Status:** Cloud sync in v0.011+.

---

## Gaps & Deferred Features

| Feature | Reason | Target Version |
|---------|--------|-----------------|
| **Presence (typing, last-seen)** | Non-blocking; low priority | v0.017+ |
| **Rich reactions & emoji** | UI polish; messages work without | v0.017+ |
| **Business channels** | Enterprise feature; not core | v0.018+ |
| **Multi-device sync** | Requires session state server | v0.019+ |
| **Full E2EE handshake** | WP16; partial in v0.015.2 | v0.016 |

---

## Implementation Notes

### Forward Secrecy (WP13-2, WP16-2)
- Polling eliminated when WebSocket connected (v0.015.2, Task 2)
- Double Ratchet ensures each message has unique key (v0.016, WP16-2)

### Media Resilience (WP15-2, v0.015.2 Task 3)
- Range-request streaming (RFC 7233) for instant seek
- DoH fallback (DNS-over-HTTPS) for CDN resilience (Task 3, in progress)

### Multi-Account (WP16-3, v0.016)
- Profile isolation per `user_id`
- Independent session/messages.db per profile
- <0.5s account switching

---

## References

- **Signal Protocol Spec:** https://signal.org/docs/specifications/doubleratchet/
- **RFC 7233:** https://tools.ietf.org/html/rfc7233 (HTTP Range Requests)
- **Zalo E2EE Analysis:** `re-notes/36-zalo-e2ee-signal-protocol-analysis.md`
- **E2EE Integration Plan:** `re-notes/38-e2ee-chat-integration.md`

---

**Last Updated:** 2026-11-08 (v0.015.2)
