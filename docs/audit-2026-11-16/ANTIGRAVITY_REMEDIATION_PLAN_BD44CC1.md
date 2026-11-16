# Kế hoạch khắc phục cho Antigravity — Audit `bd44cc1`

**Đối tượng:** Antigravity (agent triển khai). **Maintainer** đã duyệt thứ tự 4 phase dưới đây.
**Báo cáo gốc (bằng chứng, file:line, kịch bản):** [CONSENSUS_AUDIT_BD44CC1.md](CONSENSUS_AUDIT_BD44CC1.md). Phải đọc mục tương ứng ở đó trước khi sửa từng mã lỗi.
**Bắt buộc đọc trước:** `AGENTS.md`, `docs/standards/` liên quan, [ANTIGRAVITY_HANDOVER.md](ANTIGRAVITY_HANDOVER.md) (cách chạy gate, những điều phải hiểu đúng), [ANTIGRAVITY_REVIEW.md](ANTIGRAVITY_REVIEW.md) (lỗi hiểu sai trước đây).

## 0. Lưu ý trước khi làm: báo cáo trước của Antigravity sai ở đâu

Báo cáo `CONSENSUS_AUDIT_REPORT_BD44CC1` của Antigravity có các điểm sai sau. Đừng lặp lại:

1. **Số test bịa ra.** Báo cáo ghi 910 passed / 11 ignored; số thật là **909 / 0 / 13** (bản E2EE: 912 / 0 / 13). Chỉ ghi con số do chính mình vừa chạy ra, cộng `test result:` của **mọi** binary test.
2. **Kết luận “sạch, 0 regression, 100% approve” khi chưa đọc hết code.** Gate xanh không chứng minh code an toàn. C-01 và H-01 đều qua mọi gate.
3. **Sai vị trí code.** Redaction F12 nằm ở `session.rs:1319-1410`, không phải ở `logger.rs/login.rs/qr.rs`. Trích nguyên văn chuỗi phải đúng (`auth_guard.rs:143`).
4. **Đề xuất sửa phá vỡ bản sửa cũ.** TTL 15–30 giây cho F08 ngắn hơn HTTP timeout 35 giây, sẽ đưa lại lỗi gửi “add” trùng. Trước khi đề xuất timeout, phải tra deadline thật của đường đi.
5. **Phóng đại rủi ro, bỏ sót rủi ro thật.** F21 không “kẹt đến mức phải dùng Task Manager”; rủi ro thật là bypass bằng relaunch (M-02) và lỗi không hiển thị (M-04).

## 1. Quy trình bắt buộc cho mọi phase

1. Mỗi mã lỗi: ghi trigger và hành vi mong đợi, đọc lại source tại HEAD (số dòng trong báo cáo thuộc `bd44cc1`), rồi sửa nhỏ nhất ở đúng boundary.
2. Viết **regression test trước** và chứng minh nó fail trên code cũ nếu có thể. Test phải đi qua boundary thật (command/event/store/parser), không chỉ test helper. Không chạm Zalo live hay dữ liệu thật.
3. Chạy gate với môi trường cô lập:
   ```powershell
   $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
   $env:CMAKE_POLICY_VERSION_MINIMUM = "3.5"
   $env:CTDOZ_ALLOW_TEST_LOOPBACK = "1"
   $env:CTDOZ_DATA_DIR = "<thư mục tạm mới>"
   cargo fmt --all -- --check
   cargo check --locked --all-targets
   cargo clippy --all-targets --locked -- -D warnings
   cargo clippy --all-targets --all-features --locked -- -D warnings
   cargo test --locked --no-fail-fast
   cargo test --locked --no-fail-fast --features experimental-e2ee
   python -I scripts/tests/test_audit_prepush.py
   powershell scripts/audit-prepush.ps1
   ```
4. Mỗi phase làm trên một nhánh riêng (`antigravity/phase-N-...`) và commit theo từng mã lỗi.
5. Cuối mỗi phase: cập nhật [IMPLEMENTATION_TRACKER.md](IMPLEMENTATION_TRACKER.md) bằng bằng chứng thật, rồi nhờ **hai reviewer độc lập** review. Chỉ merge khi cả hai cùng APPROVE và không còn lỗi blocker/major. Ghi kết quả review vào [AUDITOR_REVIEWS.md](AUDITOR_REVIEWS.md).
6. Không tự tạo tag hay release. Không push `main` khi maintainer chưa xác nhận.
7. Không đổi command ID, payload, endpoint hay hợp đồng server khi chưa có bằng chứng E1/E2.

---

## Phase 1 — Lỗ hổng an ninh (ưu tiên tuyệt đối, phát hành bản vá sớm)

### C-01 — Crash từ xa ở parser E2EE (Critical)

**Root cause:**
- `try_decrypt_e2ee_content` (`chat.rs:8958-8962`) gọi `is_e2ee_wire_payload` cho **mọi** tin 1:1, trước khi kiểm tra metadata hay trial.
- Trong `crypto/wire.rs`, `decode_varint` trả về độ dài tới `u64::MAX`, rồi `offset + len` bị tràn ở `:371`, `:408-409` và `:475-480`.

**Việc phải làm:**
1. **Chặn sớm:** chỉ tính `is_wire` khi `has_e2ee_metadata(raw)` đúng, **hoặc** khi `e2ee_trial::enabled()` và `is_thread_e2ee(profile, peer)` cùng đúng. Văn bản thường không có metadata không bao giờ được đi vào `wire::deserialize`.
2. **Parser không bao giờ panic:**
   - Thay mọi `a + b` trên offset bằng `checked_add`.
   - Kiểm tra `len <= body.len() - offset` **trước** khi ép kiểu sang `usize`.
   - Mọi lần cắt slice dùng `body.get(offset..end)`, trả về `Err` khi không hợp lệ.
   - Giá trị varint dùng cho `u32` (counter, id) phải từ chối nếu lớn hơn `u32::MAX`, không được `as u32`.
3. Rà các hàm khác trong `wire.rs` và `e2ee_integration.rs` có cùng mẫu cộng offset hoặc cắt slice, sửa luôn.
4. **Phòng thủ thêm** (tùy chọn): bọc parse từng tin trong WS consumer (`worker.rs:1042`) để một tin lỗi chỉ bị bỏ qua và ghi log, không giết task.

**Test bắt buộc:**
- `wire::deserialize` với payload `33 22 ff ff ff ff ff ff ff ff ff 01` (base64 `MyL///////////8B`) phải trả về `Err`, không panic, ở cả debug và release (`cargo test --release` cho test này).
- `try_decrypt_e2ee_content`/`parse_chat_message_in_context` với tin 1:1 văn bản thường `MyL///////////8B` và không có metadata: tin vẫn được parse thành văn bản thường.
- Proptest hoặc fuzz: `wire::deserialize` nhận `Vec<u8>` ngẫu nhiên (ít nhất 10k case) và không bao giờ panic.

**Tiêu chí đóng:** payload PoC không còn gây panic ở mọi đường (WS, long-poll, TUI); văn bản thường không đi vào parser wire; test fuzz pass.

### H-01 — Mở file đính kèm chạy được code (High; mở lại A26)

**Root cause:** `platform.rs:95-137` dùng blocklist thiếu nhiều đuôi; `safe_name` (`media_download.rs:276-288`, `attachment_cache.rs:31-38`) để lại khoảng trắng cuối; không có Mark-of-the-Web.

**Việc phải làm:**
1. **Chuẩn hóa tên file** trong một hàm dùng chung cho cả hai nơi:
   - Lặp lại việc cắt khoảng trắng và dấu chấm ở cuối cho đến khi tên không đổi.
   - Loại `:` (NTFS stream) và ký tự điều khiển; chuẩn hóa NFKC.
   - Từ chối tên thiết bị CON, PRN, AUX, NUL, COM1-9, LPT1-9 (kể cả khi có đuôi).
2. **Đổi blocklist thành allowlist** cho thao tác “Mở”: ảnh, video, audio, pdf, txt, các định dạng Office không macro (docx, xlsx, pptx), zip/rar/7z nếu maintainer đồng ý. Đuôi ngoài allowlist chỉ được **“Hiện trong thư mục”** (mở Explorer và chọn file), không gọi ShellExecute “open”.
3. **Mark-of-the-Web:** sau khi tải xong, ghi ADS `Zone.Identifier` với nội dung `[ZoneTransfer]\r\nZoneId=3\r\n`. Lỗi ghi ADS chỉ log, không chặn việc tải.
4. Áp dụng cho mọi caller: `worker.rs:1693, 1744, 1777, 1805`.

**Test bắt buộc:** chuẩn hóa `"evil.exe ."`, `"evil.exe. . "`, `"a.hta"`, `"CON.txt"`, `"x.txt:stream"`. Mọi đuôi nguy hiểm trong báo cáo (hta, cpl, msc, jar, chm, scf, appinstaller, msix, appx, appref-ms, application, settingcontent-ms, library-ms, search-ms, diagcab, psm1, ps1xml, ws, wsc, sct, xll, iso, img, vhd, vhdx, docm, xlsm) phải ra quyết định “không mở”. File được phép mở thì có `Zone.Identifier` (test trên NTFS temp).

**Nghiệm thu native (maintainer):** trên Windows 11 ARM64, nhận file tên `x.exe .` và `a.hta`, bấm Mở: không có gì chạy, Explorer hiện file.

---

## Phase 2 — Toàn vẹn tài khoản và quyền riêng tư

### M-01 — Liên hệ bị khóa gửi tin vĩnh viễn (Medium-High)

1. **Không ghi `e2ee_required` chỉ vì nhận được metadata** (`chat.rs:8972-8973`) trong build không gửi được E2EE. Thay vào đó đặt cờ trong RAM và hiện banner “Hội thoại này có tín hiệu E2EE từ Zalo; ứng dụng chưa hỗ trợ”. Quyết định chặn plaintext dựa trên metadata chỉ được thực hiện khi có bằng chứng E1/E2 (ghi vào ADR).
2. **Trial** (`e2ee_trial.rs:538-541`, `worker.rs:1376`) vẫn được ghi requirement, nhưng phải có thao tác **“Gỡ yêu cầu E2EE”** trong drawer: xác nhận hai bước, xóa dòng `e2ee_required` (và session trial nếu người dùng chọn), ghi log audit.
3. **Lỗi DB e2ee** (`e2ee_integration.rs:103`, `outgoing.rs:117`): chỉ fail lần gửi đó với thông báo rõ, không coi là yêu cầu E2EE vĩnh viễn.
4. **Test:**
   - Nhận tin có `isE2EE=1` trong build mặc định thì vẫn gửi plaintext được (kèm banner).
   - Bật trial rồi gỡ thì gửi plaintext lại được.
   - Lỗi DB thì chỉ lần gửi đó báo lỗi, các lần sau bình thường.
5. Cập nhật ADR-0036 phần “recovery path”.

### M-02 — Vượt app lock bằng thoát rồi mở lại (Medium)

1. Nếu khóa Windows Hello đang bật và phiên được khôi phục khi khởi động thì `lock_guard` khởi tạo ở **trạng thái khóa** (`auth_guard.rs:31-40`). Không hiện danh sách hội thoại trước khi mở khóa.
2. Thêm nút **“Đăng xuất tài khoản”** trên `render_lock_overlay` (`gui.rs:5780-5857`). Nút này phải chạy đúng routine logout chung (xem M-03): xóa state trong RAM, `forget_session`, rồi về màn hình QR. Tuyệt đối **không** mở khóa nội dung.
3. Ctrl+Q và Quit từ tray khi đang khóa vẫn được phép thoát (không giữ người dùng lại), vì lần mở sau sẽ khóa sẵn.
4. **Test:** khởi tạo app với session đã nhớ và lock bật thì `is_locked()` đúng ngay frame đầu. Nút đăng xuất trên màn khóa gửi `WorkerCmd::Logout` và không gọi `unlock()`.

### M-03 + L-03 — Đăng xuất không triệt để (Medium / Low-Medium)

1. Tạo **một routine logout dùng chung** cho GUI và TUI. Thứ tự: dừng runtime cũ, `profiles::forget_session(owner)` (xóa `accounts/<sha>/session.dat` và mục registry), `storage::remove_session()`, `session.logout()`.
2. TUI (`tui.rs:1884`) gọi routine này; giữ chặn logout khi `critical_inflight > 0`.
3. GUI popover (`modals.rs:240-256`): logout **hai pha**. Hiện “Đang đăng xuất…” và chỉ chuyển UI khi nhận `GuiEvent::LoggedOut`. Nếu lỗi thì worker gửi `GuiEvent::LogoutFailed { message }` (event có kiểu, không dùng `StatusUpdate`), UI giữ trạng thái cũ và cho thử lại.
4. `Session::logout` (`session.rs:558-594`, A-07): nếu dựng lại client lỗi thì không giữ client cũ (dùng client không có cookie hoặc trả lỗi); reset `domains`, `imei` theo đúng chính sách hiện có, cùng service resolver; giữ `tcp_nodelay(true)`.
5. **Test:** sau khi đăng xuất từ TUI, file session của tài khoản và mục profile không còn; khi `forget_session` lỗi thì nhận `LogoutFailed` và UI không đổi.

### M-05 — File tạm riêng tư không bị xóa (Medium)

1. Chuyển WAV ghi âm (`worker.rs:3485-3508`) từ `%TEMP%` vào thư mục tạm dưới root tài khoản (`storage::root()`/account dir, có tôn trọng `CTDOZ_DATA_DIR`).
2. Ảnh clipboard/chụp màn hình (`gui/messages.rs:168-174`, `gui.rs:2400/3099/3169`, `workspace.rs:3386`) cũng dùng thư mục tạm của tài khoản.
3. Xóa file khi: tin được server chấp nhận (`ServerAccepted`; giữ lại nếu tin còn cần retry), người dùng hủy draft, logout hoặc đổi tài khoản. Khi khởi động, quét và xóa file tạm cũ hơn 24 giờ.
4. Xóa `qr.png` (`qr.rs:292-294`) ngay sau khi đăng nhập thành công hoặc hủy QR.
5. **Test:** gửi xong thì file biến mất; logout thì thư mục tạm của tài khoản trống; file của tài khoản A không nằm ở đường dẫn tài khoản B thấy được.

### M-07 — Link preview lộ IP và giả danh Discordbot (Medium)

1. **Tin nhận được** (`gui/messages.rs:782-786`): chỉ được dùng metadata từ `parselink` của Zalo hoặc metadata có sẵn trong tin. **Không** gọi `scrape_opengraph_metadata`. Gợi ý: thêm tham số `origin: LinkOrigin { Incoming, Outgoing }` cho `WorkerCmd::ParseLinkPreview` và `parse_link_metadata`, rồi chỉ chạy nhánh scrape khi `Outgoing` và cài đặt cho phép.
2. **Tin người dùng gửi** (`outgoing.rs:214`, xem trước trong composer `modals.rs:3245`): được scrape trực tiếp nếu cài đặt **“Tự tải xem trước liên kết”** đang bật (mặc định: bật cho tin gửi đi).
3. **User-Agent** (`chat.rs:2818-2820`): đổi thành `Mozilla/5.0 (compatible; ctdozlite-linkpreview)`, không kèm phiên bản hay thông tin máy. Không giả danh bất kỳ dịch vụ nào.
4. Giữ nguyên mọi lớp chống SSRF hiện có (redirect policy, `PublicResolver`, giới hạn 128 KiB, timeout).
5. **Test:**
   - Tin nhận được có link thì không phát sinh request nào tới host của link (dùng server loopback đếm request); chỉ gọi đường `parselink`.
   - Tin gửi đi thì có scrape.
   - Chuỗi User-Agent không chứa “Discord”.

### L-02 — Che thông tin nhạy cảm trong log (Low-Medium; F12 về “Một phần”)

1. Viết lại `scrub_sensitive_tokens` (`session.rs:1365-1410`) thành tokenizer key/value. Phải bắt được các dạng `key=v`, `key: v`, `"key":"v"`, `"key": "v"`, `key="v"`, `'key':'v'`, không phân biệt hoa thường.
2. Không bao giờ cắt chuỗi ở giữa ký tự UTF-8 (dùng `char_indices` hoặc `is_char_boundary`).
3. **Test:** payload JSON có `zpw_enk`, `zpw_sek`, `cookie`, `token`; chuỗi `cookie:"ĐĐĐĐĐĐ` không panic; proptest chuỗi Unicode ngẫu nhiên không panic.

---

## Phase 3 — Hiển thị lỗi và hiệu năng

### M-04 — Lỗi không hiển thị (Medium; liên quan F21)

1. Kiểm kê khoảng 95 điểm phát `GuiEvent::StatusUpdate` (`worker.rs`, `call.rs`, `media.rs`, `gui.rs`). Lỗi người dùng cần biết thì chuyển sang toast (`show_toast`); thông tin tiến độ giữ nguyên là status.
2. Thêm vùng hiển thị status trong `render_lock_overlay` để lý do lỗi Windows Hello (`AppUnlockFailed.message`) hiện ngay dưới nút.
3. **Test** event→state→render: `AppUnlockFailed` thì overlay có chuỗi lỗi; một `StatusUpdate` loại lỗi tiêu biểu thì có toast.

### M-06 — I/O đồng bộ trên runtime 2 luồng (Medium)

1. Chuyển SQLite/DPAPI/fsync ra khỏi executor:
   - `history_sync.rs:248, 263-315`;
   - `chat.rs:2025, 2281` (upload checkpoint);
   - `worker.rs:2154-2160, 4156-4161` (`save_to_file`: clone session, **nhả mutex**, rồi `spawn_blocking`);
   - `e2ee_integration::with_manager` (`e2ee_integration.rs:20, 92`) khi được gọi từ ngữ cảnh async.
2. Ưu tiên một **store actor** dùng chung cho mỗi tài khoản, cùng mô hình với TUI `StoreWriter`, thay vì `spawn_blocking` rải rác.
3. **Test:** test chứng minh socket ping/heartbeat vẫn chạy đúng hạn khi có một thao tác SQLite chậm, dùng store giả có sleep.

### L-01 — Ghi snapshot toàn bộ và scheduler trong command loop (Low-Medium; F06 mở rộng sang GUI)

1. `persist_threads_async` (`gui.rs:2057-2064`, 20 call site): không clone toàn bộ thread trên render thread. Gửi delta (các thread hoặc tin thay đổi, kèm revision) và gộp snapshot liên tiếp, chỉ giữ bản mới nhất.
2. Ghi (`worker.rs:1953-1967`) chạy trong store actor, không `await` nối tiếp trong command loop.
3. Ticker `RunAutomation`/`PruneExpired` (`worker.rs:1297-1298, 1457-1487`): tách ra task riêng dùng `MissedTickBehavior::Skip`, dùng kết nối store dài hạn.
4. `MessageStore::open` (khoảng 72 call site): cache một kết nối cho mỗi tài khoản. Migration chỉ chạy một lần, trong `BEGIN IMMEDIATE` có đọc lại `user_version` (sửa luôn A-10).
5. **Đo lường bắt buộc:** p95/p99 của thời gian từ click mở thread và từ gửi tin, trước và sau, với 500 thread x 200 tin. Ghi số vào tracker.

### Hiệu năng phụ (Low)

- **Sidebar** (`workspace.rs:4-42`): cache key sắp xếp theo revision, không quét message mỗi frame.
- **Font CJK** (`theme.rs:933-983`, B-13): đọc font trong background rồi swap qua event; chỉ kiểm CJK trên text mới.
- **Voice** (B-11): `reserve` trước bộ đệm mẫu, tránh copy khi chuyển `Arc`.
- **Logger:** đổi `Vec::remove(0)` sang `VecDeque`.

---

## Phase 4 — Dọn dẹp, test còn thiếu, nghiệm thu native

### Sửa nhỏ (Low)

| Mã | Việc |
|---|---|
| A-08 | `validate_ws_host` yêu cầu scheme `wss`; lỗi DNS thì fail-closed, có timeout; `CTDOZ_ALLOW_TEST_LOOPBACK` chỉ có hiệu lực khi `cfg(debug_assertions)`. |
| A-09 | `xeddsa_sign` báo lỗi khi RNG fail; bỏ các encoding/MAC legacy không còn cần; `root_from_secret` từ chối secret ngắn; zeroize khóa (crate `zeroize`). |
| B-09 | Tính idle cả MouseWheel, Touch, PointerMoved (debounce). |
| B-10 | Ghi âm dùng micro người dùng chọn (fallback về default); preview hỗ trợ output I16/U16. |
| B-12 | Khi chạm giới hạn 5 phút/64 MiB mà người dùng đã bấm Gửi thì vẫn gửi. |
| B-14 | Mọi lời gọi `ShellExecuteW` chạy trên một thread STA riêng có `CoInitializeEx(COINIT_APARTMENTTHREADED \| COINIT_DISABLE_OLE1DDE)`. |
| L-04 | Sửa doc comment `wire.rs:178` và mọi chỗ ghi “standard Signal” thành “envelope tùy biến E3, không tương thích libsignal”, cho tới khi có known-answer vector. |
| F10 | `e2ee_security_summary` phản ánh cả trạng thái session missing/pending/failed và phạm vi media (chỉ text). |

### Dọn code rác (chứng minh không còn caller trước khi xóa)

- Bỏ phụ thuộc `rand_core` (`Cargo.toml:46`).
- Ngừng sinh khóa Ed25519 legacy cho định danh mới; giữ đọc để tương thích dữ liệu cũ (`keystore.rs:26-52`, `store.rs:217`).
- Xóa: alias `generate_djb`; `e2ee_integration::decrypt_message`/`is_e2ee_message` chỉ dùng trong test; hằng lệnh Signal 540–582 và `prekey::build_*_payload` không dùng; nhánh `WorkerCmd::RetryOutgoing { .. } => {}` (`worker.rs:1621`).
- `has_header_injection` và `Impact::for_account_switch`: xóa, hoặc dùng thật trong production.
- Thay các test chỉ phản chiếu implementation (`tests/audit_regressions.rs:528-594`, B-15) bằng test boundary thật: `WorkerCmd::SwitchProfile` rồi reset state ở `GuiEvent::ProfileSwitched`.
- **Script:** `audit-prepush.ps1` đọc file với `-Encoding UTF8`; `build-windows-arm64.ps1` thêm `#Requires -Version 7`.
- Thay `std::thread::spawn` gọi từ UI (`gui.rs:2373, 2386`, `modals.rs:1501, 1515`, `avatar_panel.rs:123`, `tools_panel.rs:1239`) bằng `WorkerCmd`, theo AGENTS.md, hoặc ghi rõ ngoại lệ được phép.

### Test còn thiếu cho 9 finding “Một phần”

| Finding | Test cần có |
|---|---|
| F02 | Fixture đổi tài khoản / khởi động lại với hidden chat trong TUI. |
| F03 | Qua `StoreCommand::Delete` → `DeleteDone`; DB lỗi sau ACK (fault injection). |
| F04 | Event cũ đến sau logout tại boundary `check_background_events`. |
| F05 | Ma trận reorder ACK / echo / restart trong TUI. |
| F06 | Đo p95/p99 (Phase 3). |
| F08 | Capture request dispatch; stale completion; drop guard phát `ReactionFailed` khi task panic (không dùng TTL ngắn hơn 35 giây). |
| F09 | Benchmark voice trước/sau. |
| F20 | Test command/state cho popover tài khoản. |
| F21 | Test M-02 và M-04 ở trên. |

### Nghiệm thu native Windows 11 ARM64 (maintainer thực hiện, Antigravity soạn checklist)

1. **Windows Hello:** khóa, mở khóa bằng khuôn mặt/vân tay/PIN; hủy; để quá 30 giây; thoát rồi mở lại khi đang khóa (phải khóa sẵn); nút đăng xuất trên màn khóa.
2. **Micro:** ghi 1 phút và 5 phút, chọn micro khác mặc định, chạm giới hạn trong lúc đã bấm Gửi.
3. **File đính kèm:** `x.exe .`, `a.hta`, `doc.docm`: không file nào được chạy. `ảnh.jpg` mở được và có Mark-of-the-Web.
4. **Mở thư mục** có tên tiếng Việt và dấu cách; COM/ShellExecute không lỗi.
5. **Popover tài khoản** ở DPI 150% và 200%, cửa sổ hẹp.
6. **Link preview:** nhận link trỏ về server log riêng thì server **không** nhận request khi mở hội thoại.

Ghi biên bản vào tracker ở cột “Native / official”, tách riêng với “code/fixture verified”.

---

## Mẫu báo cáo cuối mỗi phase (Antigravity điền)

```text
Phase:
Nhánh / commit:
Mã lỗi đã sửa (commit tương ứng):
Mã lỗi còn mở / một phần và lý do:
Test mới (tên test, boundary nào, fail trên code cũ? yes/no):
Lệnh gate đã chạy (lệnh chính xác, exit code, passed/failed/ignored cộng mọi binary):
Đo lường (nếu có, kèm phương pháp):
Reviewer 1 / Reviewer 2 (kết luận, lỗi, vòng sửa):
Native ARM64: PENDING / kết quả
Rủi ro còn lại:
Tracker đã cập nhật: yes/no
```
