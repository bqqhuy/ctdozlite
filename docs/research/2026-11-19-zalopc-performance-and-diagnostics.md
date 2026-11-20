# Báo cáo nghiên cứu: hiệu năng UI/UX, gửi media và chẩn đoán ZaloLite

Ngày 19/11/2026 · Phạm vi: Zalo PC 26.11.12 và ZaloLite tại HEAD `0cce17a` · Trạng thái: nghiên cứu, chưa sửa mã

## 1. Kết luận điều hành

ZaloLite hiện không chậm vì một điểm duy nhất. Độ trễ cảm nhận được hình thành từ việc nhiều công việc đúng về mặt an toàn đang nằm trên đường đi của thao tác người dùng: kiểm tra quyền nhóm, lấy metadata, phân tích link, chuẩn bị ảnh, ghi tệp clipboard, mở picker native và chờ request/ACK. Zalo PC tách rõ ba việc:

1. cập nhật UI cục bộ ngay khi người dùng gõ/chọn tệp;
2. chuẩn bị thumbnail/metadata ở worker có thể hủy;
3. upload và xác nhận server ở phía sau, với trạng thái riêng.

Fork đã có optimistic message cho text và có `Attachment draft` cho kéo-thả, nhưng nút chọn ảnh/file hiện vẫn đi thẳng vào `PickAndSendAttachment`. Vì vậy fork chưa nhất quán với trải nghiệm PC: thao tác chọn tệp không tạo bản nháp trong composer để người dùng xem trước, bỏ chọn hoặc thêm chú thích trước khi gửi.

Mục tiêu hiệu năng nên là **instant local feedback**, không phải giả tạo ACK nhanh. Thành công cần đo riêng: thời gian từ phím Enter đến bubble pending, thời gian đến request start, thời gian đến echo/ACK và thời gian đến trạng thái cuối.

## 2. Nguồn và mức bằng chứng

| Mã | Nguồn | Mức | Dùng để kết luận |
|---|---|---:|---|
| PC-E2-01 | `C:\Users\bqqhu\AppData\Local\Programs\Zalo\Zalo-26.11.12\resources\app.asar`, renderer `pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`, SHA-256 `78801014154a86bade096eb916e541223a5f553f888882aa0ef3a218a44d6c08` | E2 | Luồng local message, file/photo picker, phonebook, thumbnail và logging trong bundle |
| PC-E2-02 | `re-notes/44-map15-network-send.md`, `re-notes/48-pc-261112-phase3-phase4-gap-extraction.md`, `re-notes/49-upload-limits-pc-261112.md` | E2 + fixture | Queue, preview budget, upload chunk và giới hạn kích thước |
| PC-E1-* | Các capture được ghi trong `re-notes/` | E1 ở từng ghi chú | Contract đã bắt thật; không suy rộng sang route khác |
| FORK-01 | Graphify graph tại commit `0cce17a`, `src/gui/workspace.rs`, `src/gui/worker.rs`, `src/gui/messages.rs` | Code evidence | Đường đi thực tế hiện tại của fork |
| LIVE-UNKNOWN | Test native cùng tài khoản/thiết bị | Chưa có | RTT, effective config PC, DNS/failover thực tế, crash root cause |

`app.asar` là nguồn cài đặt Zalo PC trên máy; không đưa nguyên bundle, session, DB hoặc dữ liệu tài khoản vào repository.

## 3. Zalo PC làm gì để tạo cảm giác tức thì

### 3.1 Tin nhắn text

Trong renderer PC, hàm gửi gọi `addLocalMessage` trước khi xử lý server. Bundle ghi rõ các bước `addLocalMessage successfully`, dispatch `SEND_MSG` và sau đó mới chạy `sendMsgObject`. Đây là optimistic local commit có identity `clientId/msgId`, không phải ACK giả. Fork cũng đã làm optimistic UI trong `src/gui/worker.rs`: gửi `GuiEvent::OutgoingState` trước `outgoing::send`, rồi persist nền. Điểm cần đo tiếp là fork còn chờ gì trước khi nhận `WorkerCmd::SendMessage` và lúc group policy cache miss.

### 3.2 Ảnh/file/video

PC mở file/photo dialog, kiểm tra resource và tạo message model có `localPath`/`clientId`; bộ xử lý media có progress, checksum cache, thumbnail/dimension worker và local message trước upload. Với photo, `localPath` được giữ cho message ngay trong bước xử lý resend/local state. Với file, PC khởi tạo `FileChatModel` trạng thái Upload và cập nhật phần trăm.

Fork hiện có hai đường khác nhau:

- Kéo-thả: `src/gui/file_drop.rs` tạo `Attachment draft`, đọc stamp/size và thumbnail 64×64 ở worker. Đây gần mô hình PC.
- Nút attach: `request_attachment_picker()` gửi `WorkerCmd::PickAndSendAttachment`; worker mở picker rồi gọi `send_attachment_batch()` ngay. Đây là nguyên nhân kiến trúc khiến nút attach không có preview ổn định trong composer.

Đề xuất parity: nút attach phải tạo `Attachment draft` giống kéo-thả; chỉ khi người dùng bấm Gửi mới chuyển draft thành `OutgoingJob`. Chọn nhiều tệp phải hiển thị danh sách thumbnail/icon, kích thước, lỗi đọc cục bộ và nút bỏ từng tệp. Không gửi tự động chỉ vì dialog đã đóng.

### 3.3 Clipboard

Fork đã tránh đọc clipboard trên UI thread: `request_clipboard_image()` gửi `ReadClipboardImage`, worker gọi `read_clipboard_image_to_png()`. Tuy nhiên `finalize_clipboard_rgba_image()` hiện ghi toàn bộ PNG rồi mới tạo thumbnail. Với ảnh lớn, người dùng phải chờ encode + disk I/O trước khi thấy preview. Đường nhanh hơn là tạo thumbnail từ RGBA trước, phát event preview ngay, sau đó ghi bản gốc/PNG ở worker; lỗi ghi file chỉ làm attachment không gửi được, không làm mất thumbnail.

Bundle PC có paste interception trong editor và đường `getClipboard`/native image handling. Việc PC hiện thumbnail tức thì là bằng chứng hành vi UI, chưa đủ để kết luận API clipboard nội bộ cụ thể.

### 3.4 Mở ảnh gốc

Fork đang render thumbnail clipboard trong composer nhưng chưa có interaction mở ảnh gốc tương đương viewer. PC dùng local/blob URL hoặc media viewer; fork cần giữ `Attachment draft` và mở viewer từ bản local đã xác thực, không tải lại từ server khi file vẫn còn trên máy.

## 4. Nút thắt cụ thể trong fork

### P0 — Nút attach gửi ngay

`src/gui/workspace.rs::request_attachment_picker` và `src/gui/worker.rs::PickAndSendAttachment` ghép picker với send. Điều này trái với yêu cầu preview trong composer và làm các lần bấm lặp trước đây tạo cảm giác treo. Guard picker trong `0cce17a` đã chặn dialog trùng, nhưng chưa đổi lifecycle thành picker → draft → send.

### P0 — Group policy trên đường đi của send

`group_state::snapshot()` fail-closed và cache 15 giây. Đây là quyết định an toàn, không được bỏ. Nhưng khi cache miss, composer bị khóa/chờ policy. Cách đúng là warm-up khi chọn thread, khi thread xuất hiện trong sidebar và sau reconnect; send vẫn kiểm tra snapshot hiện hành nhưng không bắt người dùng chờ lần đầu sau khi nhấn Enter.

### P1 — Metadata đọc quá rộng trong một chu kỳ

`account_metadata::read()` chạy `tokio::join!` cho privacy, labels, archive, aliases, friends, profile và inbox; worker chỉ phát snapshot sau khi các future bounded hoàn tất. Điều này khiến tên/alias và trạng thái UI phụ thuộc tác vụ chậm nhất. Tách các snapshot thành các event độc lập: cached aliases/friends trước, network refresh sau; phần thất bại không được làm chậm tên đã có.

### P1 — Tên danh bạ chưa theo mô hình PC

Bundle PC có `getPhonebookV2(phonebook_version)` với cache local `phonebook_key`, `phonebook_ver_key`, `time_update_pbook`; kết quả là map UID → display name. Fork hiện dùng alias map và profile/friend APIs, chưa có consumer `phonebook/v2`. Vì vậy không được nói fork đã đồng bộ danh bạ điện thoại. Cần triển khai sau khi chốt contract E1 hoặc fixture đã khử dữ liệu.

### P1 — Realtime fallback còn có polling

Fork ưu tiên WebSocket nhưng fallback `PollMessages` chạy mỗi 1,2 giây khi WS down. Đây không phải nguyên nhân của độ trễ khi WS khỏe; cần log rõ trạng thái epoch, số lần fallback và thời gian từ push đến render. Không giảm polling interval mù để bù cho lỗi WS.

### P1 — Preview link nằm trong critical path có giới hạn

`re-notes/44-map15-network-send.md` ghi fork đã có budget preview 2 giây, queue/policy/request/total budget. Đây là lựa chọn bounded và an toàn, nhưng cần đo tỷ lệ cache hit/fallback. Hết budget phải gửi text thường đúng identity, không chờ scrape tiếp.

### P2 — Render/layout

Black divider và settings vỡ là vấn đề bounds/viewport cần screenshot native theo kích thước và DPI cụ thể. Không thể kết luận từ bundle route. Regression nên đo rect liên tục của sidebar/chat/drawer/composer ở 1020×700 và compact widths, sau đó pixel-check renderer CI.

## 5. Kiến trúc UX đề xuất

### 5.1 State machine cho Attachment draft

```text
Idle
  -> Picking
  -> DraftPreparing (size, stamp, type, thumbnail)
  -> DraftReady (thumbnail/icon, caption, remove, open)
  -> UploadPending (optimistic bubble + progress)
  -> Sent | DeliveryUnknown | Failed
```

Quy tắc:

- `Picking` có single-flight theo account/runtime/thread.
- `DraftPreparing` không chặn text composer; mỗi file có trạng thái riêng.
- Ảnh/video phát thumbnail trước; file PDF/Excel/Doc/PPT/Zip/khác dùng icon xác định loại.
- `DraftReady` không phát network mutation.
- `UploadPending` tạo local message một lần, dùng cùng Local message ID qua mọi phase.
- Timeout sau khả năng server đã nhận là `DeliveryUnknown`, không tự gửi lại.

### 5.2 Fast path cho text

1. UI tạo draft snapshot và Local message ID.
2. Phát bubble pending ngay trong cùng frame.
3. Persist SQLite nền.
4. Worker chạy policy snapshot đã warm; nếu thiếu, policy request có deadline riêng.
5. Gửi một mutation; reconcile echo/ACK/history theo identity.

Không chạy thumbnail, OpenGraph scrape, alias refresh hoặc group roster trên đường UI → pending bubble.

### 5.3 Fast path cho clipboard

1. Đọc clipboard trên worker.
2. Giới hạn pixel/byte trước decode.
3. Tạo thumbnail từ RGBA và phát `AttachmentDraftReady`.
4. Ghi bản gửi ở worker với tên account-scoped.
5. Viewer đọc bản local; không chờ upload.

## 6. Cơ chế log toàn diện nhưng an toàn

### 6.1 Ba tầng log

**Tầng A — Product event log, luôn bật, JSONL, bounded**

Ghi state transition không chứa payload: `ui_action`, `composer`, `attachment`, `outgoing`, `incoming`, `metadata`, `ws`, `call`, `crash`. Mỗi event có `ts`, `runtime_id`, `account_tag` HMAC, `thread_tag` HMAC, `intent_tag`, `phase`, `elapsed_ms`, `result_class`.

**Tầng B — Network timing, opt-in hoặc dev mode**

Mở rộng `Session::send_phase_timings`: `service_key`, candidate index, DNS/connect/TLS/request/body/ACK, cache hit, retry count, HTTP class, `error_code`. Chỉ log host đã allowlist dạng service/candidate index; không log query, cookie, UID hoặc URL đầy đủ.

**Tầng C — Crash/native diagnostics**

Giữ `last-panic.txt`, `last-native-error.txt`, phase ring và thread/runtime metadata. Thêm breadcrumb cuối cùng cho UI/worker/media/picker nhưng không cấp phát hoặc lock trong native exception handler.

### 6.2 Event schema đề xuất

```json
{"v":1,"ts":"...","kind":"attachment","phase":"draft_ready","runtime":7,"account":"hmac:…","thread":"hmac:…","intent":"hmac:…","file_kind":"image","bytes":1048576,"thumb_ms":18,"result":"ok"}
```

Các trường cấm: `zpw_sek`, `zpw_enk`, cookie, IMEI, password, phone number, message text, caption, file path, raw URL, raw params, media bytes, viewer key.

### 6.3 File và rotation

- `diagnostics/events.jsonl`: tối đa 8 MiB hiện tại + 1 file trước.
- `diagnostics/network-timings.jsonl`: tối đa 4 MiB + 1 file trước.
- `diagnostics/crash/*`: chỉ report bounded.
- Writer là thread riêng với bounded channel; khi đầy phải ghi counter `dropped_events`, không chặn UI/Worker.
- `flush()` chỉ dùng khi thoát hoặc người dùng xuất gói chẩn đoán.

Logger hiện tại đã có redaction tốt cho request preview và `transfer_diagnostics` đã có bounded writer; cần hợp nhất taxonomy và thêm product events thay vì tạo logger thứ ba không liên kết.

## 7. Bộ đo hiệu năng bắt buộc

| Metric | Điểm bắt đầu | Điểm kết thúc |
|---|---|---|
| `text_local_feedback_ms` | Enter/click Send | bubble pending hiển thị |
| `text_queue_wait_ms` | enqueue | Worker bắt đầu xử lý |
| `text_policy_ms` | bắt đầu policy | snapshot cho phép |
| `text_request_ms` | request start | body/ACK |
| `text_delivery_ms` | intent | Sent/Unknown/Failed |
| `attachment_picker_ms` | click attach | dialog closed |
| `attachment_thumb_ms` | file accepted | thumbnail/icon ready |
| `attachment_draft_to_send_ms` | click Send | upload started |
| `upload_first_progress_ms` | upload started | progress đầu |
| `upload_total_ms` | upload started | complete |
| `alias_cache_hit_ms` | thread list ready | tên hiển thị |
| `phonebook_refresh_ms` | request start | version applied |
| `ws_push_to_render_ms` | frame receive | bubble render |
| `ui_frame_budget_ms` | frame begin | frame end |

Mỗi metric cần p50/p95/p99, cache hit rate, lỗi theo class và số mutation/intent. Không dùng thời gian screenshot/tool làm RTT.

## 8. Lộ trình triển khai và tiêu chí nghiệm thu

### Gói 1 — Instrumentation trước

Thêm product event schema, bounded JSONL writer, intent correlation và đo các phase trên. Không đổi protocol. Nghiệm thu: test không lộ secret; log dropped event; account switch không trộn runtime.

### Gói 2 — Attachment draft parity

Đưa nút picker về `Attachment draft`; dùng chung với kéo-thả; thumbnail/icon và viewer local. Nghiệm thu: chọn file không upload; cancel không mutation; nhiều file không mở dialog trùng; click Send tạo đúng một intent.

### Gói 3 — Optimistic text/media và warm-up

Warm group policy khi chọn thread; phát cached alias/phonebook trước refresh; giữ fail-closed. Nghiệm thu: Enter hiển thị pending trong cùng frame; policy miss không làm mất draft; echo/ACK không tạo duplicate.

### Gói 4 — Clipboard fast path

Thumbnail trước disk encode, bounded decode, viewer local và cleanup account-scoped. Nghiệm thu: ảnh lớn vẫn hiện thumbnail trước khi PNG hoàn tất; lỗi ghi file không crash; cancel không để file rác.

### Gói 5 — Native benchmark và parity audit

Đo Zalo PC và fork cùng máy, cùng loại text/media, cùng account test được phép. Ghi effective host/config, p50/p95/p99, số retry và kết quả delivery. Chỉ sau bước này mới điều chỉnh timeout/retry.

## 9. Những điều chưa được khẳng định

- Chưa có E1 đo RTT gửi tin của Zalo PC và fork trong cùng điều kiện.
- Chưa biết effective `retry_config`, active domain và feature flags của phiên Zalo PC hiện tại.
- Static bundle chứng minh local-message/progress/phonebook code tồn tại, không chứng minh mọi nhánh runtime luôn chạy như vậy.
- `getPhonebookV2` được bundle xác nhận là map UID → display name; chưa có fixture tài khoản để chốt cách fork nên áp dụng vào mọi contact.
- Crash clipboard của fork chưa có dump/phase log xác định; không nên kết luận là `image` decoder hay picker trước khi có event timeline.

## 10. Quyết định nghiên cứu

**Parity với Zalo PC/Web:** local message trước network, upload progress, phonebook cache/version, thumbnail/dimension worker và service-map routing (nguồn: PC-E2-01, `re-notes/44`, `re-notes/49`).

**Fork deviation có chủ đích:** fail-closed group permission, không retry mutation mù, không log raw payload, không lưu session trong diagnostics (nguồn: AGENTS.md, standards security/concurrency, `re-notes/44`).

**Unknown — AMBIGUOUS:** effective native retry, DNS rotation trigger, viewer implementation details và nguyên nhân crash thực tế. Không dùng các mục này làm contract server.


## 11. Phu luc dieu chinh theo phan hoi loi nguoi dung (19/11/2026)

Phan nay tach roi cac hien tuong tung bi gop chung thanh "mat mang" hoac "cham", doi chieu truc tiep code fork tai 0cce17a.

### 11.1 "Cho hang doi" (QueueWait) khong phai loi mang

- Code: src/outgoing.rs::record_queue_wait + src/service_resolver.rs (QUEUE_BUDGET = 5s, TimingPhase::QueueWait, SendBudgetElapsed).
- Y nghia that: enqueued_at -> thoi diem worker bat dau xu ly intent. Vuot 5 giay nghia la INTENT NAM TRONG HANG DOI WORKER (worker ban thumbnail/metadata/upload khac hoac channel day), khong phai RTT hay WS.
- Nguyen nhan kha di trong code: permit UPLOAD_JOBS giu den khi UI nhan, download_media_batch chay tuan tu, worker xu ly tuan tu tung WorkerCmd.
- Huong dung: do text_queue_wait_ms rieng (da co phase), them worker_busy_depth, giam cong viec dai chiem slot worker truoc khi cham budget.

### 11.2 Upload xong nhung "khong co duong dan tep" khac mat mang

Chuoi that khi gap loi nay:
1. post_multipart tra HTTP 200 nhung BODY RONG -> send_multipart_body (src/session.rs) tu tong hop {"error_code":0,"data":null}.
2. confirmed_upload (src/chat.rs:1758) yeu cau data la object/array -> data:null bi tu choi voi loi "May chu chua xac nhan du lieu gui".
3. Neu data co nhung thieu fileUrl: fork cho control file_done/video_done qua upload_events::wait toi da ASYNC_FILE_URL_WAIT = 90s; het gio -> "Chua nhan duoc duong dan tep da xu ly".
4. Neu khong co fileId hop le, fork dung send_resp = {"error_code":-1} -> confirmed_send tu choi "May chu chua tra ID xac nhan gui tin".

Ket luan: day la duong server xu ly bat dong bo chua cap URL (hoac envelope khong dat chuan), thuoc lop ProtocolFailure/DeliveryUnknown - byte da upload xong, khong duoc retry mu theo re-notes/44. UI/log hien gop cac nhanh thanh mot thong diep; can tach ma: upload_204_empty, no_file_done_control, no_file_id.

### 11.3 Mat mang / offline

- src/session.rs: chi ma server 600/-102 lam het phien; loi connect/timeout giu phien va credential (valid/expired/unreachable phan tach). Failover chi replay mutation khi DNS preflight failure hoac connection refusal TRUOC KHI GUI; HTTP loi, body stall, timeout -> DeliveryUnknown, khong gui lai.
- WS down -> fallback PollMessages 1,2s; offline van giu lich su tu SQLite. Khoang trong: khi offline keo dai, composer cho soan nhung moi send fail NotSent - can hien thi "Offline - tin se khong gui duoc" thay vi cho budget.

### 11.4 Duong dan tai tep ve va mo tep goc

- Anh/video nhan: src/attachment_cache.rs dat dich sha256(thread.len():thread:message)-safe_name trong thu muc rieng tai khoan, budget 512 MiB/7 ngay, Entry::reserve chong tai trung, tep partial giu 24h, ready() chi mo tep hoan chinh.
- Luu/tai batch tu kho media: src/gui/worker.rs::download_media_batch gioi han 100 muc, nguoi dung chon thu muc dich qua native dialog, co MediaStarted/MediaProgress va nut huy.
- Tin gui di giu local://<path> cho anh local (src/chat.rs:2294). Khoang trong: PC 26.11.12 co enable_predownload_file:false + max 30 MiB auto-download (re-notes/48); fork chua co consumer predownload tuong duong - la gap tinh nang, khong phai loi.

### 11.5 Goi thoai / video call

- Trang thai: PureRustCallState (per-call), TerminalCalls account-scoped chong re-ring sau endcall, GuiEvent::CallStateChanged cap nhat state_label. Camera/mic qua command 12006 /api/voicecall/conf; nhom qua bo 12433-12447.
- Gap da ghi nhan (khong phai loi moi): fork khong gui RTP header extension (TWCC) va RR toan so 0, khop viec peer bao "mang kem" trong capture re-notes/08. Do bang call_trace.log (opt-in, chua payload signaling - khong commit).
- Gioi han an toan: goi nhom bi chan o call_target_allowed tru khi bat khoa test; khong tu chuyen nhom thanh nhieu cuoc goi 1-1 khi mo phong that bai.

### 11.6 Clipboard

- Doc: Ctrl+V -> request_clipboard_image() gui WorkerCmd::ReadClipboardImage; src/clipboard_image.rs mo clipboard tren worker thread, ho tro DIB/PNG/HDROP, phase marker clipboard-decode, gioi han pixel/byte.
- Anh dan hien thi qua pending_clipboard_image + texture "clipboard-preview"; chup man hinh qua src/screen_capture.rs cho sequence number clipboard doi roi di cung duong doc anh.
- Diem cham muc 3.3: finalize_clipboard_rgba_image() encode PNG toan bo truoc khi tao thumbnail. That bai mo clipboard ("Clipboard dang duoc ung dung khac su dung") hien chi hien text, chua co event result_class de thong ke.

### 11.7 Phan loai loi de xuan them vao schema muc 6.2

| result_class | Y nghia | Diem code |
|---|---|---|
| queue_overrun | intent cho worker > 5s | outgoing.rs::record_queue_wait |
| upload_204_empty | HTTP 200 body rong, khong xac nhan duoc envelope | session.rs::send_multipart_body |
| no_file_done | het 90s cho control URL | chat.rs ASYNC_FILE_URL_WAIT |
| no_file_id | upload response thieu fileId hop le | chat.rs asyncfile path |
| delivery_unknown | mutation khong the xac nhan, cam retry | re-notes/44, send_failure_kind |
| offline_kept | phien giu nguyen khi mat mang | session.rs expiry classification |
