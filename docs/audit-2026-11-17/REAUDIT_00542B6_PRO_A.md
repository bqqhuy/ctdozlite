# Tái kiểm toán 00542b6 — Pro A: bảo mật, phiên và vòng đời dữ liệu

Snapshot đọc: `main`, `00542b68272496087f505443b4c4804bc93a2b38`; diff so với `f2b5e9736c26dd81f4ea9342eaabf7594dac124c`. Working tree sạch lúc bắt đầu. Ngày 17/11/2026.

**Kết luận: CHANGES REQUIRED.** Các bản vá sửa được những nhánh cụ thể của N01–N05, nhưng chưa đủ đóng toàn bộ. Có regression xóa tệp gốc của người dùng. Đây là kiểm toán source, không chứng nhận toàn dự án hết lỗi hoặc Zalo interoperability.

Đã đọc AGENTS.md, skill Karpathy, các chuẩn security/crypto, concurrency, API/error handling, testing, Zalo Reference Authority và báo cáo đồng thuận trước. Graphify không có `graphify-out/graph.json` trong checkout này; tra trực tiếp source và owning boundaries. Không sửa source/Git, không đọc credential, không gọi Zalo, không tạo crash payload, không chạy Cargo trong agent này. Agent chính phụ trách gate với data root tổng hợp. Các tài liệu approval cũ chỉ là dữ liệu cần kiểm chứng.

## Finding có căn cứ source

### A01 — P1: Cleanup gửi tệp có thể xóa tệp gốc ngoài thư mục app

- Vị trí bản vá: `src/outgoing.rs:340–346`.
- Entry point: `src/gui/worker.rs:3984–3990` nhận file picker/preset; `:5624–5682` giữ đường dẫn gốc trong `Payload::Attachment` hoặc `HdSource`.
- Trigger: người dùng chọn một tệp như `D:\Work\temp\clipboard_report.png` bằng nút đính kèm; thao tác gửi trả thành công. Không cần file này do app tạo.
- Lỗi: cleanup chỉ kiểm basename bắt đầu `clipboard_` và parent có component cuối `temp`. Không kiểm root app, owner hay quyền sở hữu bản sao. Với HD, `orig_path` chính là `hd_source.path`, tức nguồn gốc ngoài app; với gửi nguyên bản, nó là đường dẫn người dùng chọn. Lệnh `remove_file` xóa tệp đó sau gửi.
- Tác động: mất tệp nguồn của người dùng. Xếp P1 vì thao tác thông thường có side effect phá hủy dữ liệu gốc, dù điều kiện tên/thư mục giới hạn số người gặp. Không diễn đạt thành remote exploit hay đã xóa tệp thật khi audit.
- Cần sửa: mang provenance/lease của temp app qua payload, chỉ cleanup tài sản app sở hữu và đúng account. Kiểm tra path là phòng vệ bổ sung, không suy quyền xóa từ tên tệp.
- Test còn thiếu: picker file ngoài root khớp tên, HD và original, file tài khoản khác; tất cả phải còn nguyên. Test hiện tại chỉ dọn account temp nên không phát hiện lỗi này.

### A02 — P2: TUI có thể vẫn hiện hội thoại sau thông báo đăng xuất thành công

- Vị trí: `src/tui.rs:1892–1915`, `:2413–2450`, `:2633–2642`.
- Phần sửa đúng: có `LogoutDone`, owner + RuntimeStamp, pending và critical-task drain qua `shutdown_writer` (`:901–921`). Không còn báo thành công ngay khi vừa spawn cleanup.
- Trigger: một poll/refresh/hydrate đã khởi chạy, người dùng xác nhận logout; cleanup đủ chậm để event cũ tới trước `LogoutDone(Ok)`.
- `run_confirm` xóa threads nhưng giữ session active/login, runtime cũ và đặt `poll_inflight=false`. `spawn_poll` (`:1286–1298`) không kiểm `logout_inflight`. Gate event chỉ kiểm RuntimeStamp, nên late `ThreadsRefreshed`/`PollDone` được nhận và xây lại threads/filter. Sau đó `LogoutDone(Ok)` chỉ gọi `session.logout()` và đặt status; không xóa dữ liệu vừa quay lại.
- Display đã được đối chiếu: `ui_draw` (`:2835–2860`) render `filtered` không kiểm login. `selected_thread_allowed` (`:1052`) qua `thread_allowed` (`:1014`); `Session::logout` (`src/session.rs:566`) thay bằng `Vault::default`, có `enforced=false` (`src/hidden_chat.rs:44–56`). Vì vậy không có gate render khác tự cứu nhánh này; ordinary chat và nội dung đã repopulate có thể còn trên màn hình sau logout.
- Cần sửa: chặn commands/events dữ liệu trong logout pending, tách terminal cleanup khỏi gate runtime đã revoke, xóa private state tại completion theo invariant rõ. Giữ retry/thông báo lỗi cleanup thật.
- Test còn thiếu: queue `ThreadsRefreshed`/`Hydrated` trước LogoutDone, failure, q ngay sau logout, drain timeout. Test `perform_account_logout` không kiểm TUI boundary.

### A03 — P2: Hydrate E2EE policy mới thực hiện SQLite và chờ mutex ngay trong render

- Vị trí: `src/gui/workspace.rs:5917–5918`.
- Trigger: mở panel thông tin hội thoại mà peer chưa ở `trial_peers` RAM, đặc biệt sau restart.
- Call chain: `is_thread_e2ee` → `check_thread_e2ee` → `with_manager` (`src/e2ee_integration.rs:16–35,76–81,125–126`) → global blocking mutex, có thể mở/migrate E2EE DB rồi đọc ratchet và marker (`src/crypto/session.rs:226–233`). Đây là synchronous work trong paint, lặp lại mỗi frame cho peer không có trong RAM.
- Tác động: UI phải chờ crypto mutex/disk/SQLite, vi phạm INV-3; có thể giật/đứng khi storage hoặc tác vụ E2EE bận. Chưa đo thời gian đứng cụ thể, không claim deadlock đã tái hiện.
- Cần sửa: worker trả snapshot policy theo account/runtime/peer, UI chỉ render; không thay hydrate bằng database query trong paint.

### A04 — P2: Clipboard temp vẫn có nhánh mất chủ khi worker/event channel bị hủy

- Vị trí: `src/gui/worker.rs:2139–2147`; constructor sweep tại `src/gui.rs:1555–1558`; `src/storage.rs:234–258`.
- Phần sửa đúng: result được GUI xử lý nhưng stale owner/thread được cleanup (`src/gui.rs:2512–2529`); result đọc xong trong worker nhưng account inactive cũng được cleanup. Không giữ lại claim rằng những nhánh này chưa sửa.
- Nhánh còn thiếu: closure `spawn_blocking` ghi PNG, trả tuple PathBuf, không giữ RAII cleanup. Khi outer task bị hủy do runtime teardown trước khi resume sau `.await`, nhánh inactive không chạy. Tương tự result đã queued bị drop khi GUI thay `event_rx` (`src/gui.rs:3229`, `:3306`) chỉ drop PathBuf. Cleanup account chạy trước revoke/runtime teardown (`src/gui/worker.rs:2320–2349`), nên closure đang ghi có thể tạo tệp sau lần clear.
- Sweep chỉ chạy một lần ở constructor, root temp + account có sẵn lúc đó, và chỉ tệp hơn một giờ. Khởi động ở màn hình QR owner rỗng rồi đăng nhập/switch không gọi lại sweep; tệp account cũ không được bao phủ bởi global/legacy scan. Do đó sweep không chứng minh vòng đời đã khép kín.
- Tác động: bản sao ảnh riêng tư có thể nằm lại trên disk. Không khẳng định tài khoản khác thấy ảnh trong UI. Lỗi xóa file cũng là best-effort, chưa có thống kê native.
- Cần sửa: ownership/lease theo file có cleanup khi cancel/drop/closed channel; orphan sweep chỉ trên temp app sở hữu và phải phân biệt file đang được draft/retry tham chiếu.
- Test còn thiếu: cancel outer task trong lúc blocking read/encode; drop receiver có queued image; restart không có active account; đảm bảo file gốc/account khác không bị xóa.

### A05 — P2: Regression test logout không tự cô lập data root

- Vị trí: `tests/audit_regressions.rs:772–802`, đặc biệt `:791` và `:800`.
- Test tên `...isolated` chỉ dùng UID giả; không cấu hình `CTDOZ_DATA_DIR`/root riêng. `.cargo/config.toml` cũng không cấu hình isolation.
- Trigger: chạy trực tiếp test hoặc `cargo test --locked` khi môi trường chưa đặt `CTDOZ_DATA_DIR`.
- `storage::default_root` (`src/storage.rs:13–26`) chọn `%LOCALAPPDATA%\ctdozlite`; `perform_account_logout` (`src/profiles.rs:167–172`) luôn gọi `remove_session`, xóa **global** `session.dat` (`src/storage.rs:298–310`) dù UID là giả. Test còn ghi registry thật qua `forget_session`.
- Tác động: test có thể xóa phiên lưu thật của developer, vi phạm chuẩn không đọc/ghi session/DB người dùng. Không thực thi nhánh này trên dữ liệu thật trong audit. Root audit đã được nhắc dùng thư mục tổng hợp.
- Cần sửa: test process/harness khởi tạo root temp trước mọi storage access; tránh env mutation race trong suite và giữ helper assertion không chạm global user state.

## N04: chính sách reset đã sửa đến đâu

`remove_e2ee_requirement` (`src/e2ee_integration.rs:104–120`) nay xóa marker và ratchet, rồi kiểm `requires_e2ee == false`. UI có xác nhận (`src/gui/workspace.rs:5929–5966`), completion kiểm owner (`src/gui.rs:4348–4361`), worker trả failure khi join lỗi (`src/gui/worker.rs:1625–1644`). Đây là sửa thật cho root cause “session vẫn buộc E2EE sau reset”. Test mới `test_n04_e2ee_reset_drops_both_marker_and_ratchet_session` chứa ratchet thực cục bộ, không chỉ marker.

Không báo `let _ = delete_session` tự động là plaintext bypass: nếu session còn, postcondition vẫn trả lỗi. Không có bằng chứng cross-account từ completion mới. Tuy nhiên đây là local downgrade có xác nhận, chưa phải parity Zalo. UI hỏi “Xác nhận gỡ E2EE?”, không nói rõ mất ratchet; nên diễn đạt hậu quả cho người dùng rõ hơn. DB failure giữa các bước, stale/closed channel, next-send thật qua boundary và restart vẫn cần kiểm chứng. A03 khiến N04 tổng thể chỉ **partial**, dù predicate cũ đã được sửa.

## Đối chiếu N01–N05

| Mục | Trạng thái | Nhận định |
|---|---|---|
| N01 | Partial, còn lỗi source | Terminal/drain đã có; A02 vẫn phá invariant private state sau logout. |
| N02 | Partial, còn lỗi source | Tách `_sprite.png`, fetch khi thiếu metadata và prune là tiến bộ; Pro B xác định legacy atlas khi animation tắt và late static completion vẫn còn. Đã tự đọc các owning lines để xác nhận. |
| N03 | Partial, có regression P1 | Nhánh stale được sửa, nhưng A01 xóa source và A04 không có ownership cleanup đầy đủ. |
| N04 | Partial | Marker+ratchet/postcondition/confirm/owner sửa đúng; A03 đưa disk/mutex vào UI, các failure/restart boundary còn thiếu. |
| N05 | Partial | DB history và cache write được offload; `FilteredMemo::load/save` vẫn sync trên async (`src/history_sync.rs:233,284,354,377`). QR save mới detached, lỗi lưu chỉ log (`src/gui/worker.rs:2564–2568,2582–2586`) thay vì StatusUpdate. |

Hai QR snapshots được tạo trước/sau `get_login_info` và `sync_profile_me`; hai job detached không có ordering/terminal ownership. `SESSION_IO` cùng active-check trong `Session::save_to_file` là phòng vệ thực, không được bỏ qua khi phân tích. Rủi ro snapshot cũ hoàn thành sau snapshot mới cần regression có điều khiển; chưa chạy interleaving probe, không khẳng định đã tái hiện khôi phục credential sau logout. Việc mất thông báo lỗi lưu phiên có thể đọc trực tiếp từ diff.

## Phản biện chéo và đồng thuận

Pro A và Pro B độc lập tìm cùng A01, A02, A03 và đã đối chiếu call chain. Pro B xác nhận cleanup có thể xóa user-original, pending logout cho phép repopulate, render policy blocking. Pro A kiểm lại `ui_draw`/Vault default để loại giả thuyết render có auth gate cứu N01.

Pro A đồng ý N02 của Pro B: preload `src/gui/worker.rs:663–674` vẫn gửi atlas legacy như ảnh tĩnh; `src/gui/messages.rs:49–50` không nâng metadata khi animation tắt. `StickerSpriteReady` clone TextureHandle (`src/gui.rs:4335–4342`), late `StickerImageReady` gọi `store_texture`→`texture.set` (`:4319–4323`, `:2161–2162`) nhưng để nguyên frames/delays, nên UV có thể cắt ảnh thumbnail với metadata atlas cũ. Đồng ý N05 memo I/O còn trên executor. Không cần suy đoán hợp đồng CDN để xác nhận các lỗi lifecycle cục bộ này.

**Đồng thuận của Pro A: CHANGES REQUIRED**, ưu tiên P1 A01; N01–N05 không được đóng toàn bộ bằng test helper hoặc lời approval cũ. Giữ 21 finding gốc theo phạm vi đã đóng trong báo cáo trước, không mở lại parser recursion/texture-bound đã sửa chỉ vì thiếu nghiệm thu native. Các gate do agent chính chạy được ghi riêng; report này không mượn số test snapshot cũ làm kết quả hiện tại.
