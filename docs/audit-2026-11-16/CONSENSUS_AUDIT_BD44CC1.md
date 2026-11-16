# Báo cáo kiểm toán đồng thuận — `bd44cc1` (main)

**Ngày:** 16/11/2026 · **Phạm vi:** toàn bộ repository tại `bd44cc1` · **Chế độ:** chỉ đọc, không sửa code
**Quy trình:** hai auditor độc lập (Opus) → phản biện chéo từng phát hiện của nhau trên source → hợp nhất. Mức độ cuối cùng do hai bên thống nhất; chỗ còn khác biệt được ghi rõ.
**Đầu vào tham khảo:** báo cáo Antigravity `CONSENSUS_AUDIT_REPORT_BD44CC1_2026_11_16.md` (được kiểm chứng ở mục 4, không coi là sự thật).

| | Auditor A | Auditor B |
|---|---|---|
| Trọng tâm | Crypto/E2EE, session, socket, SQLite, TUI, log, URL | GUI/worker, concurrency, voice, Win32, hiệu năng, script |
| Lệnh đã tự chạy | `cargo test --locked --no-fail-fast`: 909 passed / 0 failed / 13 ignored; PoC A-01 (debug + release) | `cargo test`: 909/0/13; `clippy -D warnings`: exit 0; `fmt --check`: exit 0; prepush `-SkipCargo`: CHECKED=5 SKIPPED=3 HEURISTIC=1 |

## 1. Kết luận

Bản đồng thuận Antigravity nói `bd44cc1` “sạch, 0 regression, 100% approve”. **Kết luận này không đúng.** Các gate (fmt, clippy, test) quả thật xanh, nhưng hai auditor độc lập cùng xác nhận:

- **1 lỗi Critical:** một peer gửi một tin văn bản thường là làm chết luồng nhận tin realtime của GUI, và lỗi nhiều khả năng lặp lại sau mỗi lần khởi động.
- **1 lỗi High:** mở file đính kèm nhận được có thể chạy code, vì blocklist đuôi file vượt qua được.
- **7 lỗi Medium / Medium-High:** liên quan khóa liên lạc vĩnh viễn, bypass app lock, đăng xuất không triệt để, rò file riêng tư, lỗi bị ẩn, và I/O đồng bộ trên runtime 2 luồng.

Cả hai không coi các lỗi này là regression của vòng P1–P3. Phần lớn có từ trước và nằm ngoài phạm vi 21 finding F01–F21.

## 2. Phát hiện mới (đã phản biện chéo)

### Critical / High

**C-01 — Critical — Crash từ xa, không cần người dùng thao tác, lặp lại (A-01).**
- **Vị trí:** `src/crypto/wire.rs:371-372` (vòng dò PreKey), `:407-409` và `:475-480`. Phép `offset + len` không kiểm tra tràn; `decode_varint` (`:111-125`) chấp nhận độ dài `u64::MAX`.
- **Đường đi:** `parse_chat_message_in_context` (`chat.rs:6056-6064`) chạy với mọi tin 1:1 → `try_decrypt_e2ee_content` → `is_wire` được tính ở `chat.rs:8959-8962` **trước** mọi kiểm tra metadata, trial hay feature → `e2ee_integration.rs:200-202` gọi `wire::deserialize`.
- **Kịch bản:** peer gửi văn bản `MyL///////////8B`, là base64 của `33 22 ff×9 01`.
  - Bản debug panic vì tràn số ở `wire.rs:371`.
  - Bản release: phép cộng bị wrap nên qua được kiểm tra, rồi panic khi cắt slice `body[11..10]` ở `wire.rs:409`.
  - Panic giết task WebSocket consumer của GUI (`worker.rs:1042→1147`), task này không có supervisor. Cursor hàng đợi ở `worker.rs:1167` không được commit, nên sau khi khởi động lại, tin đó nhiều khả năng được giao lại và crash lặp lại; Zalo có giao lại hay không là suy luận E3.
  - Long-poll (`chat.rs:5714/5726`) và TUI (`tui.rs:1290/1311`) cũng dính.
  - Chỉ ảnh hưởng hội thoại 1:1; người lạ có gửi được hay không tùy cài đặt riêng tư Zalo.
- **Khắc phục:**
  - Dùng phép cộng có kiểm tra (`checked_add`) và giới hạn mọi độ dài theo phần slice còn lại.
  - Từ chối counter lớn hơn u32 thay vì ép kiểu `as u32`.
  - Chỉ deserialize khi có metadata E2EE, hoặc khi trial bật và thread là E2EE.
  - Thêm fuzz/proptest cho `wire::deserialize`; cân nhắc `catch_unwind` theo từng tin để phòng thủ nhiều lớp.

**H-01 — High — Mở file đính kèm có thể chạy code (A-03 gộp với B-01).**
- **Vị trí:** blocklist ở `platform.rs:95-137`; `safe_name` ở `media_download.rs:276-288` (và bản sao `attachment_cache.rs:31-38`); caller ở `worker.rs:1693/1744/1777/1805`.
- **Ba lỗ hổng:**
  - (a) Blocklist thiếu các đuôi: hta, cpl, msc, jar, chm, scf, appinstaller, appx/msix(+bundle), appref-ms, application, settingcontent-ms, library-ms, search-ms, diagcab, psm1, ps1xml, ws, wsc, sct, xll, iso, img, vhd, vhdx. File Office có macro cũng được mở mà không có Mark-of-the-Web.
  - (b) `trim()` chạy trước `trim_matches('.')`, nên `"evil.exe ."` thành `"evil.exe "`. Đuôi file lúc này là `"exe "`, không có trong danh sách; Win32 bỏ khoảng trắng cuối và chạy `evil.exe`. Hành vi chuẩn hóa của Win32 đã được xác nhận bằng probe .NET, nhưng chưa chạy end-to-end trên ARM64.
  - (c) Không ghi `Zone.Identifier`, nên SmartScreen và Office Protected View không kích hoạt.
- **Khắc phục:**
  - Chuẩn hóa tên: lặp lại việc cắt dấu chấm và khoảng trắng cuối, loại cú pháp NTFS stream, chuẩn hóa NFKC, từ chối tên thiết bị (CON, NUL…).
  - Đổi sang **allowlist** gồm ảnh/video/audio/PDF/văn bản/Office; các loại còn lại chỉ “Hiện trong thư mục”.
  - Ghi ZoneId=3, hoặc đi qua `IAttachmentExecute`.
  - Thêm regression test cho tên có dấu chấm/khoảng trắng cuối và cho từng đuôi nguy hiểm.

### Medium-High / Medium

| ID | Mức | Vị trí | Kịch bản | Khắc phục |
|---|---|---|---|---|
| **M-01** (A-02) | Medium-High *(A: Medium, B: Medium-High)* | Ghi: `chat.rs:8972-8973`, `e2ee_trial.rs:538-541`, `worker.rs:1376`. Chặn gửi: `session.rs:226-234`, `chat.rs:1793-1796/1822/3025`, `outgoing.rs:172`, `chat.rs:8944`. Lỗi DB fail-closed: `e2ee_integration.rs:103`, `outgoing.rs:117`. | Trong build mặc định, chỉ cần một tin có `isE2EE` hoặc `e2eeStatus` 3000–3999, hoặc một lần bật trial, là bảng `e2ee_required` ghi một dòng **không bao giờ bị xóa**. Từ đó liên hệ này không gửi được nữa: plaintext bị chặn, còn E2EE bị chặn trong build mặc định. Lỗi DB e2ee chặn luôn mọi lần gửi 1:1. | Không lưu requirement từ metadata E3 trong build không gửi được E2EE (chỉ hiện banner). Thêm thao tác “reset yêu cầu E2EE” có xác nhận. Lỗi DB thì chỉ fail lần gửi đó, kèm thông báo rõ. |
| **M-02** (B-08) | Medium | `auth_guard.rs:31-40`; `gui.rs:5384-5395` | App lock luôn khởi động ở trạng thái mở. Ctrl+Q hoặc Quit từ tray vẫn hoạt động khi đang khóa. Mở lại ứng dụng với phiên đã nhớ là thấy hết hội thoại. Đây mới là rủi ro thật của F21, không phải “kẹt vĩnh viễn”. | Nếu đã bật khóa Windows Hello và có phiên được khôi phục thì khởi động ở trạng thái khóa. Nút “Đăng xuất” khôi phục (nếu thêm) phải xóa RAM và gọi `forget_session`. |
| **M-03** (A-04) | Medium | `tui.rs:1884` so với `worker.rs:2049`; `profiles.rs:98-112, 141-160` | Đăng xuất từ TUI chỉ xóa `session.dat` ở root. Bản `accounts/<sha>/session.dat` và mục registry vẫn còn, nên account switcher của GUI có thể vào lại tài khoản đã “đăng xuất”. | TUI gọi `forget_session(owner)`; GUI và TUI dùng chung một routine logout. |
| **M-04** (B-02) | Medium | `gui.rs:5036-5038`; chỗ duy nhất đọc `status_msg` là tooltip ở `workspace.rs:474-478`; `gui.rs:3233-3246`; overlay ở `gui.rs:5780-5857` | `status_msg` gần như không bao giờ hiển thị, nên khoảng 95 điểm phát `StatusUpdate` (lưu lịch sử lỗi, mở video/liên kết lỗi…) đều vô hình. Màn hình khóa không vẽ rail, nên **bản sửa R1 “hiện lý do lỗi Windows Hello” không có tác dụng thật**. | Đưa lỗi lên toast hoặc thanh trạng thái, thêm nhãn lỗi trong `render_lock_overlay`, và test event→render. |
| **M-05** (B-04) | Medium *(A: Low-Medium)* | `worker.rs:3485-3508`; `gui/messages.rs:168-174`; `gui.rs:2400/3099/3169`; `workspace.rs:3386` | WAV ghi âm (plaintext) nằm trong `%TEMP%\ctdozlite_voice`, ngoài root tài khoản và bỏ qua `CTDOZ_DATA_DIR`. Ảnh clipboard/chụp màn hình lưu thành `media-cache/clipboard_<ms>.png`. Cả hai không bao giờ bị xóa, kể cả khi logout hay đổi tài khoản. Đây là phần nối dài của F01 ra ổ đĩa. | Lưu dưới root tài khoản; xóa sau khi gửi thành công, khi hủy, khi logout/đổi tài khoản; quét file cũ lúc khởi động. |
| **M-06** (B-07) | Medium | `worker.rs:927-929` (runtime 2 luồng); `history_sync.rs:248, 263-315`; `chat.rs:2025, 2281`; `worker.rs:2154-2160` (`save_to_file` giữ mutex); `e2ee_integration.rs:20, 92` (`with_manager`) | SQLite/DPAPI/fsync đồng bộ chạy thẳng trên executor, vi phạm chuẩn concurrency (SQLite phải qua `spawn_blocking`). Chỉ cần hai lệnh chặn cùng lúc là socket ping bị đứng, có nguy cơ chạm timeout liveness 45 giây. Kết hợp với C-01: WS consumer là điểm lỗi duy nhất. | Chuyển sang `spawn_blocking` hoặc store actor; nhả session mutex trước khi làm disk I/O. |

**M-07 — Medium — Link preview làm lộ IP người nhận và giả danh Discordbot** (bổ sung sau phản biện, maintainer đã đồng ý xếp vào Đợt 2).
- **Vị trí:** `gui/messages.rs:782-786` gửi `WorkerCmd::ParseLinkPreview` cho cả **tin nhận được** khi tin đó cuộn gần tới viewport → `worker.rs:3349-3355` → `parse_link_metadata` (`chat.rs:2912`) → `scrape_opengraph_metadata` (`chat.rs:2770`, gọi ở `chat.rs:2975`) tải HTML trực tiếp từ máy người dùng, với User-Agent `Discordbot/2.0` (`chat.rs:2818-2820`).
- **Kịch bản:** người gửi đặt một link trỏ về server của họ. Khi người nhận mở hội thoại, server đó ghi được IP thật và thời điểm mở (giống tracking pixel). Lớp chống SSRF không chặn được vì đích là host công khai. Ngoài ra, giả danh bot của bên thứ ba có thể khiến CDN chặn IP người dùng, và trái nguyên tắc trung thực của dự án.
- **Khắc phục:**
  - Tin nhận được chỉ dùng `parselink` của Zalo; thiếu metadata thì hiển thị link dạng đơn giản.
  - Chỉ tải trực tiếp cho link người dùng tự gửi, kèm cài đặt bật/tắt.
  - Đổi sang User-Agent trung thực và chung chung.

### Low-Medium

| ID | Vị trí | Vấn đề | Khắc phục |
|---|---|---|---|
| L-01 (B-05 + B-06) | `gui.rs:2057-2064` (20 call site); `worker.rs:1953-1967, 1297-1298, 1477-1486` | Mỗi lần mở thread, gửi hoặc react đều clone toàn bộ thread trên render thread, rồi ghi full snapshot nối tiếp trong command loop mà không gộp. Ticker 2 giây mở SQLite ngay trong loop. Hậu quả là lệnh gửi phải xếp hàng. Đây là vấn đề độ trễ, không phải tính đúng. | Ghi delta theo revision và gộp snapshot; tách scheduler ra task riêng (`MissedTickBehavior::Skip`); dùng kết nối dài hạn. |
| L-02 (A-06 + B-16) | `session.rs:1365-1410`; `logger.rs:139` | Redaction bỏ sót dạng JSON (`"zpw_enk":"…"`, `token="…"`). Ngoài ra có thể **panic** ở biên ký tự UTF-8 khi giá trị rỗng, ví dụ `cookie:"` theo sau là các chữ “Đ…”, làm crash task gọi log. | Viết lại thành tokenizer key/value, có test JSON và test Unicode. **F12 chuyển về Một phần.** |
| L-03 (B-03) | `modals.rs:240-255`; `worker.rs:2051-2071` | Logout từ popover đổi UI trước khi worker xác nhận. Nếu `forget_session`/`remove_session` lỗi thì UI kẹt ở “Đang xin mã QR mới…” trong khi runtime cũ vẫn chạy. | Logout hai pha; thêm event `LogoutFailed` có kiểu. |
| L-04 (A-05) | `ratchet.rs:40`; `wire.rs:152, 168-174, 178, 204-205`; `ratchet.rs:669-682` | Trial E2EE khác libsignal ở chuỗi info KDF (`…V2`), đầu vào MAC (không bind identity) và khóa 32 byte thay cho DJB 33 byte. Doc comment `wire.rs:178` ghi là “standard Signal” là sai, dù header module đã ghi E3. Tương tác thật với libsignal sẽ luôn fail MAC. | Ghi nhãn rõ “envelope tùy biến E3”, hoặc căn theo libsignal và thêm known-answer vector. |

### Low

| ID | Vấn đề (vị trí) |
|---|---|
| A-07 | `Session::logout` (`session.rs:558-594`): nếu dựng lại client lỗi thì giữ client cũ cùng cookie jar cũ; không reset `domains`/`imei`/resolver; bỏ mất `tcp_nodelay`. |
| A-08 | `socket.rs:183-203`: lỗi DNS cho qua (fail-open) và có TOCTOU; `validate_ws_host` không kiểm scheme nên `ws://` sẽ gửi cookie dạng rõ; `CTDOZ_ALLOW_TEST_LOOPBACK` vẫn có hiệu lực trong bản release (`:164, :184`). |
| A-09 | Vệ sinh crypto: `keystore.rs:123` bỏ qua lỗi RNG; chấp nhận hai kiểu encoding/MAC; `root_from_secret` tự pad 0; không zeroize khóa; clone toàn bộ session mỗi lần decrypt (`ratchet.rs:431`). |
| A-10 | `store.rs:80`: đọc `user_version` ngoài transaction nên migration có thể chạy hai lần khi nhiều kết nối mở cùng lúc lần đầu. Khả năng xảy ra thấp. Nên làm như crypto store: `BEGIN IMMEDIATE` rồi đọc lại version. |
| A-11 | `qr.rs:292-294`: `qr.png` không bao giờ bị xóa. |
| B-09 | `gui.rs:5187-5196`: idle chỉ tính phím và click, nên người đang cuộn đọc vẫn bị tự khóa. |
| B-10 | `voice.rs:141-145`: ghi âm bỏ qua micro người dùng chọn; preview chỉ hỗ trợ output F32. |
| B-11 | `voice.rs:114`: Vec tăng gấp đôi trong callback CPAL khi đang giữ mutex; `Arc::from(Vec)` copy thêm một lần (chưa được auditor A xác minh). |
| B-12 | `voice.rs:755-773` so với `workspace.rs:7212-7231`: khi chạm giới hạn 5 phút/64 MiB, ý định “Gửi” của người dùng bị bỏ (chỉ một auditor đã xác minh). |
| B-13 | `theme.rs:933-983`: đọc font CJK khoảng 20 MB đồng bộ trên render thread, và quét CJK mỗi frame. |
| B-14 | `platform.rs:3-31`: gọi `ShellExecuteW` mà không `CoInitializeEx(STA)`. Rủi ro khi nghiệm thu native. |
| B-15 | `tests/audit_regressions.rs:528-594`: test chỉ phản chiếu implementation (`Impact::for_account_switch`, `has_header_injection` chỉ dùng trong test; `sanitize_header_value` chỉ áp lên hằng số). |

## 3. Điều chỉnh trạng thái F01–F21 (cả hai đồng ý)

| ID | Tracker hiện tại | Đề xuất | Lý do |
|---|---|---|---|
| F01 | Đóng | Đóng, nhưng mở thêm M-05 | Ownership trong RAM đúng, nhưng file WAV rò ra `%TEMP%`. |
| F06 | Một phần (chỉ TUI) | Một phần, mở rộng sang GUI | Cùng loại vấn đề ở GUI: L-01, M-06. |
| F08 | Một phần | Một phần, rủi ro **thấp** | Worker trả lời trên mọi nhánh và HTTP timeout 35 giây; khóa chỉ rò khi task panic. |
| F10 | Đóng | Một phần (nhỏ) | Chưa phản ánh trạng thái session missing/pending/failed và phạm vi media như tiêu chí đóng yêu cầu. |
| F12 | Đóng | **Một phần** | L-02: bỏ sót JSON và có panic UTF-8; code redaction nằm ở `session.rs:1319-1410`. |
| F13 / F19 | Đóng (thử nghiệm) | Giữ, sửa lời mô tả | L-04: không phải “Signal chuẩn”. |
| F14 | Đóng | Đóng cho thư mục chẩn đoán; **mở lại A26** | H-01 (đường `open_file`) và B-14. |
| F21 | Một phần | Một phần, sửa bằng chứng | Bản sửa R1 hiển thị lỗi không có tác dụng (M-04); rủi ro thật là M-02; kết quả về sau 30 giây bị bỏ. |

## 4. Kiểm chứng báo cáo Antigravity

| Nhận định của Antigravity | Kết luận | Bằng chứng |
|---|---|---|
| Gate 4: “910 passed, 0 failed, 11 ignored” | **Sai** | Cả hai auditor chạy đều ra 909 / 0 / 13 ignored (bản E2EE theo tracker là 912 / 0 / 13). |
| Gate 1, 3 (fmt, clippy) PASS | Đúng | Auditor B chạy: exit 0. |
| Gate 5: CHECKED=7, HEURISTIC=1, SKIPPED=1 | Đúng (suy ra) | `-SkipCargo` cho 5/3/1; cộng 2 bước cargo thì thành 7/1/1. |
| Gate 2, Gate 6 | Chưa kiểm | Không chạy riêng. |
| “Codebase sạch, 0 regression mới, 100% approve” | **Sai** | C-01, H-01, M-01…M-06. |
| “Không có blocking I/O hay spawn thread không kiểm soát trên render thread” | **Sai** | B-13 (đọc font), L-01 (deep clone), `std::thread::spawn` từ UI ở `gui.rs:2373, 2386`, `modals.rs:1501, 1515`, `avatar_panel.rs:123`, `tools_panel.rs:1239` (trái với AGENTS.md “qua WorkerCmd”). |
| F12 nằm ở `logger.rs/login.rs/qr.rs` | **Sai vị trí, đóng quá sớm** | Code ở `session.rs:1319-1410`, `login.rs` không có redaction; xem L-02. |
| F21 “RỦI RO CAO: kẹt hoàn toàn, chỉ Task Manager” | **Phóng đại** | Timeout 30 giây bật lại nút; Ctrl+Q và Quit từ tray vẫn hoạt động; rủi ro thật là M-02 cộng M-04. |
| F21 trích thông điệp “Yêu cầu Windows Hello đang được xử lý” | **Sai** | Chuỗi thật: “Đang có yêu cầu Windows Hello khác xử lý.” (`auth_guard.rs:143`). |
| F08 “RỦI RO TRUNG BÌNH”, đề xuất TTL 15–30 giây | **Rủi ro phóng đại; đề xuất sai** | TTL ngắn hơn HTTP timeout 35 giây sẽ đưa lại lỗi gửi “add” trùng mà R1 đã sửa. Nên dùng drop guard trong task worker phát `ReactionFailed`, hoặc TTL lớn hơn deadline kèm kiểm `intent_id`. |
| Popover tài khoản duy nhất tại (68.0, 44.0) | Đúng | `modals.rs:30`, gọi một lần ở `workspace.rs:4285`. |
| Tham chiếu dòng của F11, F13, F16, F19, F21 | Đúng | Đã kiểm. |
| E2EE / Phone Sync parity “Unknown — AMBIGUOUS” | Đúng nhưng thiếu | Không nêu việc wire format lệch libsignal (L-04). |
| Lộ trình nghiệm thu native ARM64 | Hợp lý | Nên bổ sung: kiểm H-01 với tên file có khoảng trắng cuối, M-02 (relaunch khi đang khóa), B-14 (COM). |

## 5. Tối ưu và code rác

- **Phụ thuộc thừa:** `rand_core = "0.10"` (`Cargo.toml:46`) không được dùng ở đâu.
- **Legacy còn hoạt động:** khóa ký Ed25519 vẫn được sinh và lưu dù không còn dùng để xác minh (`keystore.rs:26-52`, `store.rs:217`).
- **Code chỉ phục vụ test hoặc không dùng:** alias `generate_djb`; wire v1 và MAC 32 byte; `e2ee_integration::decrypt_message`/`is_e2ee_message`; hằng lệnh Signal 540–582; `prekey::build_*_payload`; `has_header_injection`; `Impact::for_account_switch`.
- **Nhánh không bao giờ tới:** `WorkerCmd::RetryOutgoing { .. } => {}` (`worker.rs:1621`).
- **Hiệu năng crypto:**
  - Mỗi tin E2EE serialize JSON toàn bộ RatchetSession (tới 2000 skipped key) rồi DPAPI (`crypto/store.rs:497-514`). Nên dùng encoding nhị phân và bảng skipped key riêng.
  - `try_decrypt_e2ee_content` decode base64 mọi tin 1:1. Chặn sớm theo metadata sẽ vừa nhanh hơn vừa sửa luôn C-01.
- **`MessageStore::open`:** có khoảng 72 chỗ gọi, mỗi lần chạy lại DDL, pragma và migration check. Nên cache một kết nối cho mỗi tài khoản, hoặc dùng store actor.
- **Sắp xếp sidebar:** `workspace.rs:4-42` tạo lại key và quét message mỗi frame; nên gắn cache theo revision.
- **Logger:** `Vec::remove(0)` nên đổi sang `VecDeque`.
- **Script:** `audit-prepush.ps1` trên PowerShell 5.1 dùng `Get-Content -Raw` không có UTF-8 nên làm vỡ tiếng Việt ở phần heuristic. `build-windows-arm64.ps1` cần `#Requires -Version 7`.
- **User-Agent Discordbot:** maintainer đã quyết định bỏ; xem M-07.

## 6. Thứ tự khắc phục đề xuất

1. **C-01** (crash từ xa): sửa ngay. Chặn sớm theo metadata, dùng phép cộng có kiểm tra, thêm fuzz test.
2. **H-01** (thực thi file đính kèm): allowlist, chuẩn hóa tên, ghi Mark-of-the-Web.
3. **M-01** (khóa liên lạc vĩnh viễn), **M-02** (bypass app lock), **M-03** (TUI logout), **M-05** (file tạm riêng tư), **M-07** (link preview lộ IP).
4. **M-04** (lỗi vô hình, kéo theo F21), **M-06** (I/O đồng bộ trên executor).
5. L-01…L-04, sau đó các mục Low và dọn code rác.

## 7. Chưa kiểm chứng được

- Zalo có giao lại tin sau crash C-01 không; peer thật có đặt `isE2EE`/`e2eeStatus` không. Cả hai đều là E3.
- Chạy end-to-end H-01 qua Rust và `ShellExecuteW` trên Windows 11 ARM64; mọi hành vi native (Hello, micro, DPI, COM).
- Đo tác động độ trễ của L-01 và M-06 (p95/p99).
- Có thể có race thứ tự giữa persist Pending và persist cuối (`worker.rs:1322` so với `:1334`); chưa kết luận.
- Đường dẫn `purge_received` có khớp `history_key` không; `e2ee.db` không bị xóa khi logout hoặc quên tài khoản (cần maintainer xác nhận có chủ ý hay không).

> Ghi chú vận hành: auditor A dựng một PoC trong scratchpad và build với `CARGO_TARGET_DIR` của repo, nên còn sót `target\debug\poc.exe` và `target\release\poc.exe`. Thư mục `target/` đã được gitignore; có thể xóa hai file này.
