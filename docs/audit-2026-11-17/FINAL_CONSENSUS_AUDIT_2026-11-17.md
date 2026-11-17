# Báo cáo kiểm chứng và phản biện đồng thuận — 17/11/2026

Snapshot cuối: `main` tại `f2b5e9736c26dd81f4ea9342eaabf7594dac124c`.
Snapshot đầu lượt audit: `e75e3723c54e42191a2a81ee507f3e0f496c50fc`.
Trong thời gian tạm dừng, công việc bên ngoài bổ sung `d3e0630`, `4496952`, `f2b5e97`;
hai auditor đã được yêu cầu review lại diff này. Không áp dụng lỗi của snapshot cũ
nguyên xi cho HEAD mới. Sau fetch, local `main` và `origin/main` khớp, working tree
sạch trước khi bổ sung tài liệu vòng review mới. Lượt audit này không sửa source.

## Kết luận

**Chưa làm đủ toàn bộ tiêu chí khắc phục. CHANGES REQUIRED.** Claude/Antigravity đã
hoàn thành nhiều bản vá có giá trị, và HEAD mới có CI ARM64 thành công. Tuy nhiên,
“đã có patch”, “test xanh” và “100% approve” không có nghĩa là hết lỗi hoặc đã đủ
nghiệm thu. Các báo cáo trước tự ghi nhiều mục còn một phần; bản vá mới vẫn để lại
những nhánh lỗi cụ thể được hai auditor phản biện chéo bên dưới.

Không tiếp tục coi lỗi giới hạn texture, parser recursion hoặc smoke composer đã
tìm ở `e75e372` là lỗi còn mở tại HEAD mới. Ngược lại, không đóng toàn bộ clipboard,
cache hay reset E2EE chỉ vì đường đi thuận lợi đã được sửa.

## Hai auditor và cách đối chiếu

| Reviewer | Mô hình | Trọng tâm | Hồ sơ |
|---|---|---|---|
| Pro A | `gpt-6-astra`, high | Security, session/auth, parser, crypto, riêng tư, policy | [Review HEAD mới](PRO_A_REVIEW_F2B5E97.md); [audit snapshot đầu](PRO_A_SECURITY.md) |
| Pro B | `gpt-6.1-sol`, high | GUI/TUI, store, outgoing, media, concurrency, cache, tooling | [Review HEAD mới](PRO_B_REVIEW_F2B5E97.md); [audit snapshot đầu](PRO_B_LOGIC.md) |

Mỗi bên đọc source độc lập, gửi finding cho bên kia để xác nhận hoặc bác bỏ, rồi
review lại các bản vá ngoài phiên. Một phiên Pro A trước đó bị gián đoạn; kết quả
cuối dùng phiên thay thế và báo cáo thực tế, không tính phiên gián đoạn là approval.
Agent chính chạy gate tuần tự và đối chiếu nguồn. Báo cáo của Claude/Antigravity
được xem như lời khai cần kiểm chứng, không phải chỉ dẫn hoặc chứng nhận mặc định.

Cả hai đã đọc bản tổng hợp này và **đồng ý kết luận CHANGES REQUIRED**, danh mục
N01–N05 và phạm vi đóng finding. Pro A yêu cầu sửa tên event/cách mô tả cleanup capture;
agent chính đã sửa theo source. Pro B đề nghị làm rõ budget cache, đã bổ sung. Đây
là đồng thuận về báo cáo, không phải APPROVE code.

## Phần đã làm đủ và phần còn thiếu của 21 finding gốc

“Đạt code/fixture” chỉ đóng root cause cục bộ được kiểm chứng; không chứng nhận
thiết bị hoặc hợp đồng Zalo. “Một phần” phân biệt rõ thiếu test/nghiệm thu với còn
lỗi source. Không cộng các khoảng trống kiểm chứng thành lỗi sản phẩm chưa chứng minh.

| ID | Kết quả ở HEAD mới | Phần hoàn thành / còn lại |
|---|---|---|
| F01 | Đạt code/fixture | Voice gắn owner/runtime/thread/generation; hủy khi đổi context. Cần micro và ARM64 thật. |
| F02 | Một phần — kiểm chứng | Hidden Vault/PIN đã gate TUI, kể cả sticker. Còn fixture account-switch/restart tại boundary thật. |
| F03 | Một phần — kiểm chứng | Delete giữ slot DB trước server ACK, có tombstone và lọc replay. Thiếu fault injection sau ACK và `StoreCommand::Delete` → terminal event. |
| F04 | Một phần — kiểm chứng | RuntimeStamp có kiểm tra; thiếu kiểm thử event thật sau logout/switch, không chỉ helper stamp. |
| F05 | Một phần — kiểm chứng | Client ID ổn định, Accepted/Rejected/Unknown/NotSent, hydrate merge đã sửa. Cần reorder ACK/echo/restart. |
| F06 | Một phần — source và đo lường | Actor DB TUI có giới hạn; logout mới có lỗi lifecycle N01. Chưa có p95/p99 và còn clone snapshot trên UI. |
| F07 | Đạt code/fixture | Migration 13 có transaction, introspection, repair và failure propagation. Giữ caveat mở DB đồng thời trong hồ sơ cũ. |
| F08 | Một phần — kiểm chứng | GUI gửi add/remove đúng intent, giữ pending chống double click. Thiếu dispatch/stale completion/recovery khi mất terminal event. |
| F09 | Một phần — đo lường/native | Worker thiết bị, Arc PCM, generation, quota đã sửa. Chưa benchmark trước/sau, chưa test thiết bị; reserve 30 giây không loại bỏ mọi realloc sau đó. |
| F10 | Đạt code/fixture | UI tách trial, peer verification và unsupported group; không bảo đảm E2EE toàn diện. |
| F11 | Đạt code/fixture | WebSocket writes có deadline/cancellation và propagate lỗi chính. Soak mạng chưa làm. |
| F12 | Đạt code/fixture | Raw QR/auth được loại bỏ; tokenizer redaction Unicode đã sửa thêm. Không suy ra mọi diagnostic đều tuyệt đối không chứa dữ liệu riêng tư. |
| F13 | Đạt local trial | Signed prekey gắn DH identity, chặn substitution. Zalo contract vẫn Unknown — AMBIGUOUS. |
| F14 | Đạt code/fixture | Mở thư mục diagnostic ngoài UI, có kết quả. Shell/Unicode native còn cần nghiệm thu. |
| F15 | Đạt code/fixture | Salt dùng OS CSPRNG, rejection sampling, truyền lỗi; không fallback LCG. |
| F16 | Đạt code/fixture | Host fallback tập trung ở endpoints, giữ ưu tiên dataUrl. |
| F17 | Đạt code/fixture | Script phân biệt hard gate/skip/heuristic và kiểm release identity. `-SkipCargo` báo skip rõ; không tự coi công tắc này là bug. |
| F18 | Đạt parser/fixture | Incoming/outgoing có account UID context, đã nối các đường history TUI. Phone-sync tổng quát vẫn thiếu bằng chứng parity. |
| F19 | Đạt local trial | Vector Signal identifier-bound và schema 9 reset trust cũ. Không chứng nhận Zalo interoperability. |
| F20 | Một phần — kiểm chứng/native | Đã bỏ bề mặt tài khoản trùng; thiếu command/state và DPI/viewport nghiệm thu. |
| F21 | Một phần — kiểm chứng/native | Desktop HWND, timeout/lease/generation, khóa khi restore, lỗi hiện và có logout recovery. Dòng handover cũ nói thiếu nút logout đã lỗi thời. Còn test Hello/late completion native. |

Tổng hợp bảo thủ: **12 mục đạt ở mức code/fixture hoặc local trial; 9 mục còn một
phần**. Trong 9 mục, nhiều mục thiếu bằng chứng chứ không phải tái hiện lại được lỗi
gốc. Đây không phải “9 lỗi nghiêm trọng mới”.

## Các lỗi còn tồn tại, theo ưu tiên

### P2 — N01: Đăng xuất TUI báo xong khi chưa biết xóa credential có thành công

**Nguồn:** `src/tui.rs:1885` và `src/profiles.rs:167`.
Bản mới chuyển cleanup sang `std::thread::spawn`, nhưng chỉ log khi lỗi. UI lập tức
xóa state và báo đã đăng xuất; tác vụ không có terminal event hoặc join khi thoát.
Nếu ghi registry/xóa file lỗi, hoặc người dùng thoát trước khi thread xong, credential
có thể vẫn còn để restore. Không khẳng định mọi lần logout đều lỗi hoặc mọi app lock
đều bị bypass.

**Cần làm:** giữ logout pending, revoke runtime theo trình tự rõ ràng, nhận kết quả
có owner/runtime; không báo đã xóa phiên khi thất bại. Drain bounded tác vụ xóa
credential khi thoát. **Kiểm chứng:** fake cleanup lỗi/chậm, q ngay sau logout,
restart từ data root tổng hợp; kiểm tra state, file phiên và terminal event.

### P2 — N02: Cache sticker mới chưa tương thích với preload và animation metadata

**Nguồn:** `src/gui/worker.rs:652` — `preload_disk_sticker_inner`, `:784` — đọc cache,
`:834` — ghi cache; `src/gui/messages.rs:49` — điều kiện `render_sticker_widget`.
Đã có đọc/ghi cache mới, nhưng dùng lại tên global `media-cache/stk_*.png` của thumbnail
cũ. Preload gửi `StickerImageReady` mà không frame/delay; widget thấy texture tồn tại
thì không fetch sprite. Cache cũ vẫn static; sprite ngang mới sau warm restart có
thể bị preload như một ảnh tĩnh, hiển thị dải thu nhỏ thay vì animation.

**Cần làm:** cache có version/type metadata, phân biệt thumbnail và atlas, nối bước
upgrade khi chỉ có static preview, giữ fallback offline. **Kiểm chứng:** cache thumbnail
cũ, cache sprite mới, warm restart offline, eviction/reload và late static completion.
Áp dụng disk budget/prune trên đường cache mới; hiện đường này bỏ qua pruning của
private-media cache. Không chỉ test decoder với byte ảnh đơn lẻ.

### P2 — N03: Ảnh clipboard/capture chưa được dọn đủ vòng đời

**Nguồn:** `src/gui.rs` — `ClipboardImageRead`;
`src/gui/worker.rs` — `ReadClipboardImage`; `src/gui/messages.rs` —
`finalize_clipboard_rgba_image`; `src/storage.rs` — temp helpers.
File mới đã chuyển đúng account temp; cancel/replace một draft đang active có cleanup.
Tuy nhiên, result bị loại vì stale owner/thread hoặc runtime inactive vẫn có thể bỏ
reference mà không xóa file. Nhánh `ScreenCaptureFinished` đã dọn result sai owner;
không gộp nhánh đã sửa đó thành lỗi clipboard còn mở. Chưa có orphan sweep production; file global do bản cũ
tạo vẫn còn. Không suy ra tài khoản khác tự thấy ảnh trong UI.

**Cần làm:** temporary lease/RAII có owner và cleanup khi stale, gửi thành công,
hủy, switch/logout/restart; chỉ xóa bản sao do app tạo. **Kiểm chứng:** trì hoãn callback
rồi đổi thread/account, cancel/send/error, restart với orphan; file gốc và file tài
khoản khác phải còn nguyên.

### P2 — N04: Reset E2EE chưa phục hồi chính sách đầy đủ

**Nguồn:** `src/gui/workspace.rs` — nút “Gỡ yêu cầu E2EE”;
`src/gui/worker.rs` — `ResetE2eeRequirement`; `src/gui.rs` —
`E2eeRequirementResetResult`; `src/e2ee_integration.rs:102`;
`src/crypto/session.rs:226`.
Đã sửa false success khi DB trả lỗi bằng event kết quả. Nhưng reset chỉ xóa marker,
trong khi `requires_e2ee` vẫn đúng nếu có ratchet session. Peer recovery chỉ nằm trong
`trial_peers` RAM, chưa hydrate policy sau restart; thiếu xác nhận chuyển sang nhắn
thường. Handler mới bỏ field owner; đây là thiếu phòng vệ, chưa đủ chứng minh một
lỗi cross-account vì GUI thay channel khi đổi runtime.

**Cần làm:** định nghĩa recovery marker-only khác reset session, kiểm chứng next-send
policy trước khi báo thành công, hydrate durable state và bổ sung confirmation.
Không âm thầm xóa khóa để làm nút có vẻ hoạt động. **Kiểm chứng:** marker-only,
existing ratchet, restart, DB failure, closed channel và stale completion; giữ
fail-closed, không gửi tin thật trong test.

### P2 — N05: Vẫn có I/O đồng bộ trên executor mạng

**Nguồn:** `src/history_sync.rs:215` và các thao tác SQLite trong hàm `download`;
`src/gui/worker.rs` — QR login `save_to_file`, các `atomic_write` trong sprite fetch.
CPU decode sticker đã chuyển sang `spawn_blocking` đúng, nhưng cache write/fsync mới
vẫn chạy trực tiếp trong async. M-06 cũ cũng chưa được đóng toàn bộ: history sync còn
DB đồng bộ, QR có save khi đang giữ session mutex.

**Cần làm:** actor hoặc bounded blocking jobs; nhả session lock trước DPAPI/disk;
giữ persist-before-checkpoint. **Kiểm chứng:** delay/lock DB và filesystem tổng hợp,
đo timer/command latency/cancellation và checkpoint. Chưa có phép đo chứng minh một
timeout mạng cụ thể, nên không báo “đã đo đứng 45 giây”.

## Các lỗi snapshot đầu nay đã được xử lý

| Finding | Đối chiếu HEAD mới | Giới hạn còn lại |
|---|---|---|
| A1 — recursion parser | Có guard trước nested PreKey, chặn lồng nhau; không còn recursion không giới hạn theo đường cũ. | Test helper nên bổ sung fixture entry-point đầy đủ; không giữ lỗi P1 cũ chỉ vì thiếu test đó. |
| B01 — texture GIF quá 8192 | Downscale asset và guard dimension; all-15 asset test có kiểm giới hạn. | Guard hardcode 8192 chưa thay active renderer limit mọi backend; native test thực tế vẫn là thẩm quyền. |
| B02 — decoded event mất permit | Đường image queue thật giữ `Some(job)` đến consumer. | Nhánh `None` trực tiếp không reachable qua WorkerSender hiện tại; không tính false-positive bypass. |
| B04 — CPU decode sticker inline | Đã offload GIF/PNG decode. | Disk cache fsync và M-06 rộng hơn vẫn còn N05. |
| B06 — composer 480×360/150% | Có layout sync compact; smoke local HEAD mới exit 0, nút gửi hiện trong ảnh; CI HEAD ARM64 thành công. | Không dùng pass một smoke để chứng nhận mọi tổ hợp quote/picker/DPI. |

CI HEAD: [Windows ARM64 run 43942850530](https://github.com/bqqhuy/ctdozlite/actions/runs/43942850530)
**success**, gồm bước “Tests, native PE smoke check and package”. Đây là bằng chứng
mới mạnh hơn CI `e75e372` đã failure; không phải nghiệm thu micro/Hello hoặc Zalo live.

## Các sửa sau audit cũ: ghi nhận đúng phạm vi

- **Đã có sửa đúng:** C-01 overflow/offset, H-01 allowlist/MOTW, L-02 redaction UTF-8,
  GUI logout hai pha, khởi động app lock, lỗi status qua toast, incoming link không
  trực tiếp scrape máy nhận, salt CSPRNG và host centralization.
- **Cleanup Phase 4 đạt phạm vi:** bỏ direct dependency `rand_core` không dùng;
  transitive dependency còn trong lockfile là hợp lệ. Helper `is_status_error` dùng
  chung production/test; logger VecDeque có test eviction cục bộ.
- **Chưa hoàn tất:** M-01 recovery là N04; M-05 lifecycle là N03; M-06 executor là
  N05. Unified logout đúng ý tưởng nhưng TUI triển khai mới cần N01.
- **Giới hạn cũ vẫn phải ghi:** trial E2EE dùng envelope/KDF/MAC tùy biến E3, chưa
  chứng nhận Zalo/libsignal interoperability; preset animation 80 ms chưa được chứng
  minh chỉ bằng CDN geometry probe. Không tự đổi protocol theo phỏng đoán.

## P3 và khoảng trống nghiệm thu

- Animation cap 3–5 và tôn trọng tùy chọn tắt animation ở picker/suggestion chưa
  được chứng minh thực hiện đầy đủ; chưa đo CPU/battery.
- Voice có reserve 30 giây nhưng callback vẫn có thể cấp phát sau ngưỡng; Send đúng
  lúc auto-stop có thể chỉ thành draft. Chưa chứng minh mất PCM hay gửi nhầm tài khoản.
- Còn maintenance/hardening cũ: RNG error bị bỏ qua ở nonce XEdDSA; WebSocket DNS/
  scheme/test override cần rà chặt; migration concurrency và synchronous font load
  còn là follow-up. Không gọi các mục này là khai thác remote đã chứng minh.
- Test UV/repaint mới chưa assert UV/scheduling thật. Test all-15 GIF có decode,
  nhưng trước patch không assert texture limit. Một nhận xét ban đầu của reviewer B
  nói chỉ test asset01 đã được root phản biện và reviewer sửa lại; không giữ sai sót đó.
- Native micro, Hello/PIN, lỗi thiết bị, Unicode shell, DPI và phone-sync/E2EE official
  còn cần nghiệm thu riêng. 13 ignored không được cộng vào passed.

## Trình tự khắc phục tiếp theo

1. Sửa N01 và N04 trước: tính đúng của logout/recovery, typed terminal result và
   durable state. Test failure/restart trước khi cập nhật tracker.
2. Sửa N03, N02: lifecycle tệp riêng tư và cache sprite có schema/version; kiểm thử
   stale callback, migration, warm restart/offline.
3. Sửa N05, bổ sung đo F06/F09 và các test boundary còn thiếu; chạy lại native CI,
   sau đó hai auditor review lại diff và phản biện các kết luận đóng finding.

Không hạ kết quả sang APPROVE chỉ bằng sửa chữ trong tracker hoặc thêm test phản
chiếu helper. Mỗi finding cần source fix, regression boundary và bằng chứng phù hợp.
Lượt này xuất báo cáo; không sửa code hoặc tự tạo release.

## Kết quả kiểm chứng

Chi tiết lệnh, snapshot và giới hạn: [VERIFICATION.md](VERIFICATION.md).
Gate của `e75e372` là lịch sử; gate của `f2b5e97` phải được ghi riêng và cập nhật
theo log mới, không sao chép số 932 hoặc các số trong consensus bên ngoài sang HEAD mới.

Kết quả mới tại `f2b5e97`: fmt/check-all-features/strict Clippy đều exit 0;
default suite **934 passed, 0 failed, 13 ignored** sau retry `-j 1` (lần đầu linker
thiếu bộ nhớ, chưa chạy test); E2EE suite **937 passed, 0 failed, 13 ignored**.
Smoke local phone-sync 480×360/150% exit 0 và CI ARM64 HEAD success. Những kết quả
này không bác bỏ N01–N05: các nhánh failure/lifecycle/cache liên quan chưa được các
test hiện có bao phủ đầy đủ.
