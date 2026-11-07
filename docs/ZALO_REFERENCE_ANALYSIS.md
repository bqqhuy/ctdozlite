# Zalo PC / Zalo Web — Phân tích sâu & Quy tắc đối chiếu bắt buộc (Zalo Reference Authority)

> **Trạng thái**: Hiệu lực. Áp dụng cho mọi AI agent và lập trình viên.
>
> **Mục đích**: (1) Lưu trữ kiến thức phân tích sâu về Zalo PC 26.11.10 và Zalo Web
> (chat.zalo.me) đã giải mã tĩnh; (2) Bắt buộc mọi công việc liên quan code phải đối
> chiếu với Zalo gốc trước khi quyết định thiết kế. Bổ sung cho [AGENTS.md](../AGENTS.md)
> và [ROADMAP_PHASE_2.md](ROADMAP_PHASE_2.md).

---

## Phần I — Quy tắc đối chiếu bắt buộc (Mandatory Zalo-Parity Rule)

### 1. Luồng làm việc bắt buộc khi thay đổi code

Mọi AI agent trước khi viết code cho bất kỳ tính năng nào thuộc miền giao thức,
UI hội thoại, media, VoIP, bảo mật — phải thực hiện đủ 4 bước:

```
Bước 1 — ĐỌC BẰNG CHỨNG: Tra re-notes/ + docs/ZALO_FEATURE_API_GAPS.md + snapshot
         PC/Web (nếu có) để xác định Zalo gốc làm gì cho tính năng đó.
Bước 2 — ĐỐI CHIẾU: So sánh thiết kế dự kiến với hành vi Zalo gốc:
         - Endpoint, method, params, ACK/pagination policy
         - Command ID / frame codec nếu qua WebSocket
         - Trình tự UI (UC - user case) và trạng thái lỗi
         - Quyền riêng tư / entitlement
Bước 3 — QUYẾT ĐỊNH CÓ LÝ: Ghi rõ trong commit/PR/spec:
         - "Parity": làm đúng như Zalo gốc (nguồn: re-notes/NN)
         - "Fork deviation": khác Zalo gốc vì lý do X (cần ADR nếu thay đổi
           hành vi quan sát được từ ngoài)
         - "Unknown": chưa có bằng chứng — phải mark AMBIGUOUS, cấm suy đoán
Bước 4 — KIỂM NGHIỆM: Test/fixture bám fixture bắt từ Zalo gốc (re-notes/fixtures/),
         hoặc nghiệm thu thiết bị thật cho hành vi không unit-test được.
```

### 2. Ba mức bằng chứng — thứ bậc độ tin cậy

| Mức | Nguồn | Được phép dùng để |
| :--- | :--- | :--- |
| **E1** | Capture thật (traffic capture có ghi ngày, `re-notes/fixtures/`) | Chốt hợp đồng API, params, schema phản hồi |
| **E2** | Đọc tĩnh bundle PC/Web đã lưu (PC 26.11.10 `compact-app-pc.*.js`, Web `1.*.js` / `sync-v2-worker.*.js`) | Xác định tồn tại endpoint, command ID, trình tự UI, giá trị enum client-side |
| **E3** | Suy luận hợp lý từ hành vi tương tự đã xác minh | Đề xuất giả thuyết — phải gắn nhãn AMBIGUOUS, cấm cam kết nghiệm thu từ E3 |

**Cấm tuyệt đối**: Dùng E3 để khẳng định hợp đồng server; dùng route trong index
(JSON dump 421 route) để kết luận tính năng đã dùng được (route tồn tại ≠ schema/entitlement
đúng — xem `ZALO_FEATURE_API_GAPS.md` mục "Cách đọc bằng chứng").

### 3. Điểm neo kiểm tra nhanh khi code

Trước khi commit tính năng mới, tự hỏi theo thứ tự:

1. **Endpoint đúng chưa?** Host phải resolve qua `endpoints.rs` + `ServiceResolver`;
   service key phải đúng (`group_board` ≠ `group` ≠ `board` — kết quả đối chiếu v0.006).
2. **Params đúng chưa?** Đi qua `ParamCipher v2` + `signkey` + cookie; JSON string
   params cho nhóm, `toUid` cho 1-1 (theo re-notes/29).
3. **ACK/policy đúng chưa?** Mutations không tự retry; reads không coi missing data
   là danh sách rỗng; ACK missing của send cuối không phải bằng chứng thất bại giao.
4. **Enum đúng bảng nào?** Zalo có nhiều bảng enum không tương thích: reaction
   (1pet: text=1, photo=2, voice=3, sticker=4, contact=6, gif=7, location=17,
   video=18, file=19) khác archive enum và khác CLI msg_type (text=1, photo=32,
   sticker=36, video=44, file=46). Không trộn bảng.
5. **Trạng thái UI có khớp không?** Ví dụ: lỗi 230 của pin phải refetch, không tự
   ghi đè; delivered/seen của người lạ có policy riêng tư riêng.

---

## Phần II — Phân tích sâu Zalo gốc (tổng hợp từ re-notes/ + snapshot)

### 1. Kiến trúc tổng thể

| Thành phần | Zalo PC 26.11.10 | Zalo Web (chat.zalo.me) | Fork ctdozlite |
| :--- | :--- | :--- | :--- |
| Runtime | Electron (Chromium) | Browser | Rust native (egui), không Chromium |
| Realtime | WebSocket persistent | WebSocket + sync-v2-worker | Polling (→ Phase 2: WS push, v0.013) |
| Request signing | ParamCipher v2 + signkey (MD5) | Như PC | Đã parity (`src/param_cipher.rs`, `src/crypto.rs`) |
| Service map | `zpw_service_map_v3` động từ server | Như PC | Mirror trong `src/endpoints.rs` + ServiceResolver |
| Storage | Local Electron store | Browser storage | SQLite (`src/store.rs`) |
| Kích thước bundle JS chính | `compact-app-pc.*.js` (~lớn) | `1.*.js` + worker ~3.9MB | 0 (native) |

### 2. Transport & Realtime (cơ sở cho v0.013)

**Đã xác minh tĩnh (E2):**
- WebSocket là kênh chính cho push realtime; các lớp command: push `501/521`,
  pull `510/511`, control `601–604` (GLOSSARY), heartbeat `200`.
- sync-v2-worker xử lý batch đồng bộ: `checkRequestfulfilled` ghi từng `idx`,
  set `totalBatch = idx+1` khi `isLast`; `takeUndeliveredScopes` có điều kiện
  error/type/priority/retry riêng. Fork giữ quy tắc NGHIÊM hơn: mọi idx trong
  `0..last` phải tồn tại thật (ADR-0020 bounded queue catch-up).
- Mọi URL động qua service map — không hardcode host (bất biến AGENTS.md).

**Chưa xác minh (cần E1 trước khi triển khai v0.013):**
- Header framing 16 byte (`Magic 0x02`, CmdId u16…) — đang là giả thuyết E3 trong
  v0.013-plan; phải capture thật hoặc decode từ bundle trước khi code codec.
- Giá trị chính xác của `CMD 610` typing start/stop payload.
- Keepalive interval thực tế của PC (giả thuyết 30s trong kế hoạch — cần verify).

### 3. Chat & Message lifecycle

**Đã xác minh (E2, re-notes/29, re-notes/12):**
- Identity: `globalId` (server) là identity chính; `clientId` fallback phải scope
  theo sender — không hai tin khác người dùng chung một id (`phone-client:{len}:{sender}:{client}`).
- Quote/reply: `quote` trong `meta` giữ sender/senderId/mType/ts/message; navigation
  jump theo globalId (ADR-0006).
- Reaction: giữ `gMsgID`/`cMsgID` + internal msgType của tin nguồn; enum 1pet
  riêng biệt với CLI msg_type.
- Undo/recall: `chat.undo` msgType 36 (phone history) / `multi_undo` (12115) cho
  batch PC; recall phải mark recalled locally, không xóa hàng.
- Pin: nhóm qua `topic/create` với `needPin=true`, `boardVersion`; 1-1 dùng
  `oneone/create` — hai hợp đồng KHÁC nhau, không suy đoán lẫn.
- Reminder: TEXT_TOPIC=0, duration=-1, repeat NONE=0; reads dùng board_type=1
  với page/last_id; mutations không tự retry.
- Forward: tách 2 pha — prepare/copy media trước, forward request sau; forward
  tạo tin mới không kế thừa reaction/pin/unread; bảo vệ source-protected content.

**Khoảng trống fork (liên quan Phase 2):**
- Multi-forward 12440/12442 (UI selection + batch contract) — B06.
- Voice recorder + voicemp3 upload 11148/12060 — A16.
- E2EE chat lifecycle A19 (v0.016).

### 4. Media pipeline

**Đã xác minh (E2, re-notes/15, 22, ADR-0011/0018):**
- Photo URL có alias nhiều cấp; `renewLink` tĩnh cho FILE, động cho photo/video;
  CDN chính `photo.zdn.vn` / `video.zdn.vn`.
- Video message: tách poster/thumbnail khỏi video; upload async không chặn chat.
- zCloud: `/api/getCloudViewerKey` → `/cloudmedia/queue/pc/verify` →
  `/cloudmedia/downloadurls/pc/v1` — nằm ngoài index numeric 421 route (enum ID riêng).
- Media store server-side: `mediastore/list,preview,remove` (11841–11843) nhóm,
  `media/oneone/*` (12220–12222) cá nhân — B05.

**Khoảng trống (v0.015):**
- Range-request streaming (RFC 7233) cho video player nội bộ.
- Chunked resumable upload (PC dùng chunking cho file lớn — cần capture chunk size
  và API header thật trước khi code, cấm đoán 4MB/8MB).

### 5. VoIP

**Đã xác minh (E2, re-notes/06, 08, 09, ADR-0003):**
- Signaling qua `/api/voicecall/*` (12064 requestsharescreen, 12044 group/sharescreen).
- Call type codes có test neo (`call_type_codes_match_zalo_pc`).
- Signaling ≠ transport: có signaling route không có nghĩa pipeline capture/encode
  đã hoàn thành (C03 gap doc).

**Khoảng trống (v0.014):**
- WebRTC APM (AEC3/NS/AGC) — Zalo PC dùng WebRTC stack; fork cần tích hợp module
  xử lý hoặc triển khai DSP riêng; lựa chọn cụ thể phải đối chiếu snapshot PC để
  xác định version tham số trước.
- Software H.264 fallback — không có ở PC (PC dùng phần cứng); đây là **fork
  deviation có chủ đích** để pass CI headless.

### 6. Security & E2EE

**Đã xác minh (E2, ADR-0014, 0025, 0033, re-notes/13):**
- Transport security: HTTPS + ParamCipher v2 + cookies (`zpw_sek`/`zpw_enk`) —
  KHÔNG phải E2EE hội thoại.
- Khóa Sync2/V1 phục vụ sync thiết bị, không phải ratchet chat.
- E2EE chat (160xx/161xx `/api/e2ee/pc/message/*`) tồn tại trong bundle nhưng
  lifecycle khóa chưa được giải mã đủ — đứng ở mức nghiên cứu (A19).

**Quy tắc bất biến bảo mật (AGENTS.md + v0.016 kế hoạch):**
- Private key E2EE không bao giờ rời thiết bị; không log khóa dưới mọi hình thức.
- session.json/cookies/DB phải ở ngoài Git (DPAPI current-user).

### 7. Nhóm & cộng đồng

**Đã xác minh (E2, re-notes/30, 31):**
- Send policy nhóm: quyền lockSendMsg, creator/admin kiểm tra outgoing trước khi gửi.
- Poll/note/reminder: service key `group_board` riêng; poll share/vote có route
  (12085/12086/12087) nhưng RSVP/lặp/voter chi tiết chưa đủ (B04).
- Quản trị: duyệt thành viên (10064/10065), ban (11811–11813), link mời
  (11821–11825) — chưa có luồng fork (A13, A14, A15).

### 8. UI/UX parity targets (tham chiếu PC)

- Multi-window photo viewer: zoom tại con trỏ, pan, xoay, carousel — v0.015 WP15-1.
- Video player: progress scrubber, tức thì tua (range) — v0.015 WP15-2.
- Doc preview: PDF/text/code/log — v0.015 WP15-3.
- Typing indicator + read receipts realtime — v0.013 WP13-3.
- Multi-account switcher — Zalo PC hỗ trợ; fork chưa có (v0.016 WP16-3).

---

## Phần III — Ánh xạ roadmap Phase 2 ↔ khoảng trống Zalo gốc

| Bản | Gói công việc | Bằng chứng Zalo gốc | Mức bằng chứng hiện có | Việc đối chiếu bắt buộc trước khi code |
| :--- | :--- | :--- | :--- | :--- |
| v0.013 | WS push dispatcher | Worker sync-v2 (`checkRequestfulfilled`, `takeUndeliveredScopes`); push/pull/control cmd lớp | E2 (bundle) — framing 16B vẫn E3 | Capture frame thật (E1) để chốt codec + heartbeat; đối chiếu catch-up policy với `takeUndeliveredScopes` |
| v0.014 | WebRTC APM | PC dùng WebRTC stack (bundle) | E2 (sự tồn tại) | Xác định tham số APM version/`echoCanceller` từ bundle trước khi chọn crate |
| v0.014 | Software H.264 fallback | Không có ở PC | — | Ghi rõ fork deviation; nghiệm thu = CI pass, không claim parity |
| v0.015 | Range streaming | Cần tra bundle PC video player (thuộc tính `videoTaskWrapper`) | E2 chưa làm | Đọc bundle phần player trước khi chọn RFC 7233 headers cụ thể |
| v0.015 | Chunked upload | PC có async upload + renewlink | E2 một phần | Capture upload thật để chốt chunk size + API multipart contract |
| v0.016 | E2EE Double Ratchet | Route 160xx/161xx tồn tại; lifecycle khóa chưa giải mã | E2 một phần, lifecycle E3 | BẮT BUỘC capture key handshake thật (E1) + đọc lại re-notes/13; không được tự chế handshake |
| v0.016 | Multi-account | PC hỗ trợ profiles | E2 (UI tồn tại) | Đối chiếu cách PC tách storage/cookie jar trước khi thiết kế profiles/ |

---

## Phần IV — Bảo trì tài liệu này

- Khi một mục E3 được nâng cấp thành E1/E2 (qua capture mới hoặc đọc bundle mới),
  cập nhật bảng ở Phần III và ghi re-notes/NN tương ứng.
- Khi một fork deviation được chấp nhận, phải có ADR mới hoặc tham chiếu ADR cũ.
- Tài liệu này không thay thế `ZALO_FEATURE_API_GAPS.md` (gap inventory) — nó định
  nghĩa **quy tắc đối chiếu** và **kiến thức tổng hợp**; gap doc vẫn là danh sách
  khoảng trống chính thức theo phiên bản.
