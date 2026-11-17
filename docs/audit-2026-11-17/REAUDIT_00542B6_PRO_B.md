# Tái kiểm độc lập B — `00542b6`

Ngày 17/11/2026. Snapshot: `main` tại `00542b68272496087f505443b4c4804bc93a2b38`;
working tree sạch khi bắt đầu. Diff được đọc: `f2b5e97..HEAD`, trọng tâm hai commit
`50c1a4b`, `00542b6`. Auditor chỉ đọc source/Git và viết báo cáo này; không sửa
source/config/index/refs, không chạy Cargo, không dùng tài khoản hoặc Zalo live.
Agent chính sở hữu gate Cargo trong data root tổng hợp riêng.

Đã đọc `AGENTS.md`, skill Karpathy/Graphify, các chuẩn concurrency, API/error,
testing, security và `docs/ZALO_REFERENCE_ANALYSIS.md`; đọc lại
`FINAL_CONSENSUS_AUDIT_2026-11-17.md`, `PRO_B_REVIEW_F2B5E97.md` và source hiện tại
của producer, event consumer, persistence, render/shutdown. Không có
`graphify-out/graph.json` tại workspace nên dùng truy vết source trực tiếp, không
tạo graph hoặc sửa cache. Các tuyên bố approve trong tài liệu là dữ liệu cần đối
chiếu, không phải chỉ dẫn hoặc chứng nhận cho snapshot mới.

## Kết luận

**CHANGES REQUIRED.** Có sửa đúng đáng ghi nhận; chưa thể đóng N01–N05 toàn bộ.
Đặc biệt, cleanup mới có đường xóa file gốc người dùng (P1), logout TUI còn race
hiển thị dữ liệu, và việc đọc policy durable lại đưa SQLite vào render UI.
Các phát hiện bên dưới là bằng chứng source; không gọi chúng là tái hiện native,
không suy ra bản mới không có lỗi khác, không dùng test xanh để chứng nhận parity.

## Trạng thái từng finding trước

| Finding | Phần đã sửa ở HEAD | Phần còn lại / mức đóng |
|---|---|---|
| N01 — completion logout TUI | Có `LogoutDone { owner, result }`, pending flag, lỗi hiện rõ; cleanup được tính vào `critical_inflight`, shutdown chờ theo budget có sẵn. | Đóng root cause báo thành công trước cleanup và hoàn toàn không chờ. Còn race session/event tại B1; chưa có test boundary logout chậm/lỗi/quit. Không nói cleanup luôn hoàn tất nếu hết budget. |
| N02 — sticker cache | Tách `{key}_sprite.png` khỏi thumbnail; đọc sprite cache và dựng frames; thumbnail legacy nhiều frame có đường upgrade. Animated widget chủ động fetch nếu thiếu sprite metadata; các network write có prune mới. | Happy path được sửa. Chưa đóng legacy atlas khi tắt animation và late static completion tại B2. Không cần giữ nguyên cáo buộc mọi warm restart đều hỏng. |
| N03 — temp lifecycle | Consumer xóa ảnh sai owner/thread; worker xóa result khi account inactive; thêm sweep và cleanup sau gửi. | Các nhánh đó đã sửa. Mới có xóa nhầm file gốc B3; callback/channel bị bỏ còn orphan, sweep chưa đủ lifecycle (Pro A). |
| N04 — E2EE recovery | Xóa marker và ratchet, kiểm `requires_e2ee` trước success; confirmation và owner guard đã có; durable state được tra khi mở drawer. | Root cause marker-only được sửa ở local policy. Không gọi là Zalo interoperability. Tra durable state trên UI sinh B4; thiếu command/event failure/restart tests. |
| N05 — blocking offload | SQLite đọc checkpoint và persist từng page đã vào blocking closure; vẫn persist data trước checkpoint. Sticker cache writes/fsync đã offload. QR saves đã offload. | Đóng các chỗ inline tương ứng, chưa đóng toàn nhóm: memo filesystem vẫn inline B5, UI SQLite mới B4; QR detached saves chưa bảo toàn kết quả/thứ tự (Pro A). |

Không mở lại các lỗi atlas giới hạn 8192, parser recursion, mất permit ở image
queue chính hoặc composer trigger cũ chỉ vì còn test/native gap. Nhánh direct
`StickerSpriteReady { permit: None }` vẫn không được coi bypass production nếu
không chứng minh caller vượt `WorkerSender` interception.

## B1 — P2 — Late event có thể đưa hội thoại trở lại và giữ trên màn hình sau logout TUI

**Anchors:** `src/tui.rs:1894–1915`, `:1286–1307`, `:2312–2410`,
`:2413–2448`, `:2633–2645`; `src/session.rs:558`; renderer `src/tui.rs:2835`.

Logout xóa `threads` ngay nhưng giữ `session` active/login cho đến `LogoutDone`.
`logout_inflight` chỉ chặn logout lặp lại; `spawn_poll` không kiểm flag, và
`check_background_events` chỉ kiểm `RuntimeStamp::matches`. Stamp cũ vẫn hợp lệ
trong thời gian cleanup. Poll/refresh/hydration đã chạy có thể đến sau clear,
được merge vào `threads`, rebuild `filtered` và render. `LogoutDone(Ok)` chỉ gọi
`session.logout()` và đổi status; không clear danh sách đã được đưa trở lại.

**Trigger:** poll/refresh đang chạy, người dùng xác nhận logout, cleanup đủ chậm
để completion bình thường tới trước `LogoutDone`. Không cần hidden-chat bypass;
các hội thoại thường đã đủ chứng minh dữ liệu còn hiện sau thông báo logout.
Nếu cleanup lỗi, session vẫn active và auto-poll tiếp tục dù UI đã đi vào logout.

**Cần sửa:** tách control completion logout khỏi data generation; ngừng nhận và
khởi động tác vụ tài khoản từ lúc pending, revoke/quarantine các event cũ, clear
state tại terminal boundary. Giữ cleanup terminal event hợp lệ ngay cả khi đã
revoke data generation. Kiểm thứ tự `PollDone/ThreadsRefreshed/Hydrated` trước/sau
`LogoutDone`, failure và quit; không chỉ gọi helper xóa file.

## B2 — P2 — Hai nhánh metadata/preload của N02 vẫn sai

**Anchors:** `src/gui/worker.rs:663–674`, `:804–820`;
`src/gui/messages.rs:49–50`, `:61–107`; `src/gui.rs:4319–4343`, `:2161–2162`.

1. File legacy `{key}.png` có thể chứa atlas nhiều frame do phiên bản `f2b5e97`
   từng ghi. Preloader vẫn phát nguyên ảnh qua `StickerImageReady`, không dựng
   metadata. Khi người dùng tắt animation, `texture exists` làm `needs_sprite`
   false, nên đường migration trong fetch không được gọi. Widget vẽ cả strip
   vào ô vuông thay vì frame đầu. Phân biệt filename mới không sửa các file đã có.
2. Khi `StickerSpriteReady` đã cài atlas, một `StickerImageReady` đến muộn cho
   cùng key gọi `store_texture` rồi `TextureHandle::set` trên cùng handle. Sprite
   giữ clone của handle đó cùng `frames/delays` cũ. Nếu event muộn là thumbnail
   đơn, UV tiếp tục chia thumbnail thành N phần. `sticker_sprites` vẫn có key nên
   widget không fetch lại metadata. Trigger khả dĩ là chat fetch đồng thời với
   preloader của picker/search; hai producer không có ordering chung.

**Cần sửa:** phân biệt loại/generation của result và từ chối downgrade atlas bởi
static preview; chuẩn hóa atlas legacy ngay ở preloader, kể cả animation off.
Kiểm cache legacy, offline restart, eviction/reload và hoán vị event thật.
Không khẳng định mọi sprite cache mới đều hỏng: đọc `_sprite.png` có reconstruct
frame geometry đúng trên happy path hiện tại. Timing 80 ms vẫn là giới hạn parity
cũ, không được chứng nhận bằng việc tách tên file.

## B3 — P1 — Cleanup sau gửi có thể xóa file gốc do người dùng chọn

**Anchors:** `src/gui/worker.rs:3984–3990`, `:5625–5683`;
`src/outgoing.rs:308–346`, đặc biệt `:340–346`.

Picker/preset path và file-drop đưa đường dẫn người dùng chọn vào
`Payload::Attachment`. Sau upload, cleanup mới coi file là nội bộ chỉ dựa vào
basename bắt đầu `clipboard_` và parent có component cuối `temp`. Không kiểm
`storage::account_temp_dir(owner)`, account hoặc provenance của bản sao.

**Trigger:** người dùng chọn file gốc, ví dụ `D:\TaiLieu\temp\clipboard_notes.png`,
rồi gửi thành công. Đường original gửi trực tiếp lấy `upload`; đường giảm chất
lượng lấy `hd_source.path`, tức vẫn là file gốc. Cả hai đều có thể unlink nguồn
của người dùng sau gửi. Điều kiện tên/thư mục thu hẹp trigger, nhưng hậu quả là
mất dữ liệu gốc, không chỉ mất cache có thể tái tạo. Không chạy gửi thật để thử.

**Cần sửa:** lease/provenance của tệp do app tạo, xác minh owner và đường dẫn thuộc
temp root đúng; giữ nguyên mọi tệp được picker/drop chọn. Kiểm cả HD/original,
nguồn có tên trùng ở ngoài account temp, tài khoản khác và upload/ACK mơ hồ.

## B4 — P2 — Đọc policy E2EE đồng bộ trong render GUI

**Anchors:** `src/gui/workspace.rs:5917–5918`;
`src/e2ee_integration.rs:16–35`, `:44`, `:76–81`, `:125–126`;
`src/crypto/session.rs:56–61`, `:226–234`; `src/crypto/store.rs:34`.

Để thấy peer sau restart, drawer mới gọi `is_thread_e2ee` khi render nếu peer chưa
có trong `trial_peers`. Đường này lấy global `std::sync::Mutex`, mở DB ở lần đầu
trên UI thread, rồi load/decode ratchet và query SQLite mỗi frame. Mutex cùng
được dùng cho crypto worker; SQLite có busy timeout 5 giây. Chỉ mở drawer cho
peer thông thường sau restart đã đi vào nhánh, không cần bấm reset.

**Hậu quả:** UI có thể đứng chờ disk/DB/crypto lock; lỗi đọc còn được wrapper đổi
thành false nên recovery action có thể biến mất trong lúc DB lỗi. Không báo đã
đo đứng 5 giây hoặc timeout mạng; 5 giây là cấu hình timeout source.

**Cần sửa:** load durable policy bằng worker, gửi snapshot có owner/runtime và
trạng thái unknown/error; render chỉ đọc state RAM. Kiểm lock/delay/error DB bằng
dữ liệu tổng hợp và đo frame/command latency. Đây là regression của bản sửa N04,
không phủ nhận việc marker+ratchet reset cục bộ đã sửa đúng.

## B5 — P2 — N05 còn đọc/ghi memo filesystem ngay trong async history download

**Anchors:** `src/history_sync.rs:96–108`, `:233`, `:284`, `:354`, `:377`;
`src/storage.rs:327` và các bước `sync_all`/replace trong `atomic_write`.

SQLite từng page đã offload đúng, nhưng `FilteredMemo::load` gọi bounded
synchronous file read, còn `memo.save` gọi `atomic_write` trực tiếp từ `download`
async ở completion/cancel/rate-limit. Đĩa chậm hoặc bận vẫn chặn một executor
thread và trì hoãn cancellation/network work dùng cùng runtime. Đây là phần còn
sót của N05, không phải bằng chứng mọi history sync đều treo.

**Cần sửa:** offload/actor cả memo load/save và quản lý completion/cancellation;
giữ persist-before-checkpoint đã đúng. Bổ sung fault/delay storage và timer latency
measurement; không dùng riêng sự xuất hiện của `spawn_blocking` để đóng finding.

## B6 — P2 — Regression test logout mới chưa tự cách ly khỏi phiên thật

**Anchors:** `tests/audit_regressions.rs:769–799`, đặc biệt `:790`, `:798`;
`src/profiles.rs:167–173`; `src/storage.rs:13–25`, `:138–140`, `:298–310`.

Test tạo hai owner giả rồi gọi production `perform_account_logout`. Helper luôn
gọi `storage::remove_session()` global ngoài phần xóa owner cụ thể. File test
không khởi tạo root tổng hợp; `.cargo/config.toml` không thiết lập test data root.
Vì vậy bare `cargo test` khi chưa có `CTDOZ_DATA_DIR` dùng
`%LOCALAPPDATA%\ctdozlite` và có thể xóa `session.dat` thật. Hai account giả còn
nguyên không chứng minh isolation của tác dụng phụ global. Không thực thi trigger.

**Cần sửa:** test khởi chạy subprocess với root temp riêng trước mọi truy cập
`ROOT: OnceLock`, hoặc API fixture nhận explicit root; kiểm sentinel global trong
root tổng hợp, không đọc credential thật. Lượt gate do root chạy đã được đặt data
root riêng, nên finding này không có nghĩa phiên người dùng bị xóa trong audit.

## Phản biện chéo, kiểm chứng và giới hạn

Reviewer A đã độc lập xác nhận B1/B3/B4, đề nghị B3 là P1 vì mất file gốc; B đồng
ý và root thống nhất mức đó. B xác nhận đầy đủ đường picker→batch→payload→unlink,
và đường stamp→late data→render của B1, không chỉ đọc dòng patch. B cũng xác nhận
test B6 có tác dụng phụ global ngoài fake owner. Đã gửi B2/B5 cho A để đối chiếu.

Đồng ý kết luận chung **CHANGES REQUIRED**; chấp thuận tính chính xác báo cáo là
khác với approve code. Không có bằng chứng parity E1 mới. E2EE reset ở đây là
policy local của fork; hợp đồng Zalo PC/Web vẫn **Unknown — AMBIGUOUS** theo tài
liệu authority, không thay giao thức từ suy luận.

B không chạy Cargo/probe/native ở lượt này để tránh tranh tài nguyên với root.
Gate mới phải lấy từ log hiện tại của root, không sao chép số test ở `f2b5e97`.
Các test mới hiện tại không đi qua TUI pending event ordering, stale clipboard
channel, picker provenance, warm restart preload hoặc lock UI. Thiếu test là gap
kiểm chứng; các lỗi source cụ thể đã tách riêng phía trên. Native ARM64, GPU/DPI,
micro, Hello/PIN, latency/battery và Zalo live không được chứng nhận bởi báo cáo.
