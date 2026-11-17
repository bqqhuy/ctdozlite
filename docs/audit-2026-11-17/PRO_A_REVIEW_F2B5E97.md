# Pro A — Tái kiểm tra khắc phục tại f2b5e97

Ngày 17/11/2026. HEAD đã kiểm: `f2b5e9736c26dd81f4ea9342eaabf7594dac124c`; diff so với `e75e3723c54e42191a2a81ee507f3e0f496c50fc`, gồm `d3e0630` và `4496952`. Working tree sạch khi bắt đầu đọc; báo cáo này là artifact mới duy nhất của Pro A.

**Kết luận: CHANGES REQUIRED.** A1 đã sửa được root cause recursion. A2, A3 và A4 chưa đạt tiêu chí đóng đầy đủ; thay đổi TUI logout tạo regression về xác nhận xóa credential. Không đồng ý tuyên bố “100% hoàn tất” trong `CONSENSUS_AUDIT_PHASE1_4.md`; đó là báo cáo bên ngoài cần đối chiếu, không phải kết quả của lượt tái kiểm này.

Phương pháp: đọc diff và caller/callee hiện tại, đối chiếu invariant trong AGENTS và standards đã đọc ở lượt trước. Không sửa source, không Cargo/test mới, không gọi Zalo, không đọc dữ liệu tài khoản thật, không tạo payload gây crash. Agent chính chạy gates; Pro B kiểm logic/UI/media. Các kết luận tĩnh không giả làm native acceptance hoặc E1.

## Ma trận A1–A4

| Mục | Hiện trạng | Đã sửa | Còn lại |
|---|---|---|---|
| A1 / parser recursion | **Đóng root cause ở source; fixture còn hẹp** | `crypto/wire.rs:367–372` có depth budget, `:429–432` loại nested PreKey trước recursion `:472`. | Test `:726–746` gọi helper trực tiếp với depth tự truyền; chưa đi qua public `deserialize` với outer PreKey chứa nested PreKey. Không giữ P1 cũ mở chỉ vì thiếu fixture này. |
| A2 / M-05 temp privacy | **Một phần — P2** | `gui/messages.rs:215–225` ghi vào account temp; hủy/thay draft/logout/switch dọn file đang được UI giữ (`workspace:3431`, `gui:2456,2517,3175,3250`). | Stale result, file đã gửi, orphan sau crash và file clipboard global cũ chưa được dọn đầy đủ. Test mới chỉ gọi helper filesystem. |
| A3 / M-01 reset E2EE | **Một phần — P2** | UI không tự báo thành công ngay sau enqueue; worker `:1569–1589` phát result theo kết quả DB; GUI `:4330–4342` đổi state sau result. | Marker reset không xóa trạng thái yêu cầu đến từ ratchet; restart không hydrate peer; thiếu xác nhận và xử lý channel/JoinError đầy đủ. |
| A4 / M-06 async SQLite | **Mở — P2** | Decode sticker đã được offload (Pro B xác minh riêng). | `history_sync.rs` không đổi; SQLite vẫn đồng bộ trên async executor. Cache sticker mới cũng ghi/fsync trực tiếp trong async. |

## R1 — P2: Vòng đời file clipboard vẫn có đường bỏ sót

**Điều kiện và nguồn:**

- Người dùng đổi hội thoại trong khi worker đang đọc clipboard. `gui.rs:2504–2514` bỏ result khác owner/thread bằng `continue` trước khi xử lý đường dẫn; file đã tồn tại không được xóa.
- Logout/revoke runtime trong lúc `spawn_blocking` hoàn tất. `gui/worker.rs:2079–2087` chỉ gửi result nếu account còn active; nhánh bỏ result không dọn file. Task tạo file có thể hoàn tất sau cleanup logout, nên account-scoping đơn thuần không ngăn orphan.
- Sau khi gửi ảnh, `workspace.rs:4108–4126` chuyển path khỏi `pending_clipboard_image`; attachment batch `worker.rs:3918–3924` coi đó là file nguồn chung. `outgoing.rs:335–337` chỉ dọn bản HD khi có `hd_source`; không có temporary-source lease để dọn PNG nguồn sau ACK. Khi chuyển account, chỉ file đang pending trên UI được dọn.
- `storage::prune_old_temp_files` không có caller production. Không có thay đổi dọn `media-cache/clipboard_*.png` được tạo trước bản sửa.

**Ảnh hưởng:** plaintext riêng tư có thể tồn tại lâu hơn vòng đời thao tác dù đã cancel/switch/logout ở các thứ tự trên. Không khẳng định tài khoản khác đọc được qua UI hoặc Windows user khác có quyền truy cập.

**Test mới không đủ:** `tests/audit_regressions.rs:713–728` tự ghi bytes vào `account_temp_dir`, rồi gọi `clear_account_temp`. Nó không gọi clipboard production, không kiểm hai owner, không kiểm cancel/send/switch/restart/stale result. Test chứng minh helper có thể xóa file, không chứng minh lifecycle đã nối đủ.

**Sửa/kiểm chứng:** owner-bound temporary lease được chuyển qua event/outgoing job; dọn khi bỏ result, khi đã gửi xong hoặc hủy, giữ file khi retry còn hợp lệ; startup orphan cleanup và migration legacy file có giới hạn. Dùng ảnh tổng hợp và root temp, test thứ tự tạo-file/logout và đổi-thread trước result. Không xóa file gốc người dùng chọn.

## R2 — P2: Reset E2EE mới đồng bộ DB nhưng chưa đồng bộ chính sách thật

`e2ee_integration.rs:102–113` không đổi: chỉ xóa `e2ee_required`. `crypto/session.rs:226–233` vẫn trả `session.is_some() || required`. Vì vậy peer có ratchet session vẫn bị chặn plaintext sau khi UI nhận `success=true`, ẩn nút và hiện “Đã gỡ yêu cầu E2EE”. Đây là lỗi chức năng cụ thể của thông báo/đường recovery, không phải chứng minh plaintext leak.

Nút vẫn phụ thuộc `trial_peers` (`workspace.rs:5928`); set khởi tạo rỗng (`gui.rs:1385`) và chỉ thêm khi Prepare thành công (`:2320`). Sau restart, marker bền vững không tự hydrate vào recovery UI. Không có xác nhận chuyển sang nhắn thường như kế hoạch M-01 yêu cầu. Channel đóng tại `workspace:5944–5955` không có thông báo lỗi; `spawn_blocking(...).await.ok()` vẫn không phát result khi JoinError.

Result chứa owner nhưng consumer bỏ qua (`gui.rs:4331` dùng `owner: _`) và không có generation. **Chỉ coi đây là thiếu kiểm tra phòng thủ**, chưa đưa thành finding cross-account độc lập: worker reset đang await completion, GUI thay event channel khi logout/switch (`gui:3210–3212` và nhánh ProfileSwitched). Chưa chứng minh interleaving vượt các lớp đó.

Sửa bằng outcome đầy đủ, pending/retry UI và policy snapshot thật; phân biệt “xóa marker” với “reset ratchet”. Không tự xóa key/session để làm thông báo có vẻ đúng. Test marker-only, ratchet-present, restart, channel failure, DB failure và stale generation. Giữ send fail-closed khi đọc DB lỗi.

## R3 — P2: M-06 chưa được sửa; cache mới thêm synchronous fsync

`history_sync.rs:215,248,263,302,308,315` vẫn mở/query/persist SQLite trực tiếp trong hàm async. Caller `gui/worker.rs:2808–2810` chạy bằng `tokio::spawn`; runtime còn hai executor thread (`:1083`). Targeted offload của một số save-session/decode không đóng toàn bộ finding này.

Nhánh sprite cache mới gọi `storage::atomic_write` trực tiếp tại `gui/worker.rs:809,823,834,849`. Helper `storage.rs:267–290` làm disk write và `sync_all`; async decode offload không offload phần disk này. Đây là nguồn chặn runtime mới, đồng thời còn root cause M-06 cũ.

Ảnh hưởng có điều kiện khi DB/đĩa chậm: task dùng chung executor bị trì hoãn. Chưa có số đo để kết luận timeout 45 giây hay tỷ lệ chậm. Sửa bằng store actor hoặc đơn vị blocking có ownership/cancellation, giữ thứ tự data-before-checkpoint; đo timer progress/latency với DB fixture chậm được kiểm soát.

## R4 — P2: Regression TUI logout báo thành công trước khi xóa credential

Commit mới thay xử lý lỗi trước logout bằng detached `std::thread::spawn` tại `tui.rs:1885–1892`. Lỗi `perform_account_logout` chỉ log; UI luôn xóa state RAM và hiện “Da dang xuat” tại `:1905`. Không có handle/result/terminal event nối về TUI, và shutdown `:2674` chỉ chờ store writer, không chờ thread logout này.

**Trigger:** xóa `accounts/<owner>/session.dat` thất bại hoặc logout thread chưa hoàn tất khi người dùng bấm `q` và process thoát. `profiles::forget_session` trả lỗi ngay khi remove file thất bại (`profiles.rs:145–150`); credential còn trên đĩa, nhưng UI đã khẳng định logout thành công. Lần mở app sau có thể khôi phục credential còn hợp lệ nếu remember-login bật.

Đây là regression về durability/privacy ở điều kiện failure/interruption, **không phải bypass phổ quát**. Offload UI là đúng hướng; completion cần quay về actor, hiển thị lỗi và cho retry, shutdown phải chờ bounded completion hoặc giải thích chưa xóa xong. Test filesystem failure với fixture credential giả, delay latch trước xóa, kết quả UI và restart. Không dùng credential thật.

## Thẩm định phạm vi tuyên bố và test

- A1 root cause sửa đúng; không kéo dài finding cũ bằng cách đánh đồng “thiếu integration fixture” với “parser vẫn recursion vô hạn”.
- A2 test helper không kiểm clipboard thật hay cả lifecycle; “cách ly và dọn triệt để” vượt bằng chứng.
- A3 có terminal event ở luồng DB bình thường là tiến bộ; không có test mới qua UI/worker/event/policy boundary trong diff.
- B01 test production `decode_gif_sprite` cho toàn bộ bundled assets là đúng boundary hơn probe hình học cũ. Đánh giá renderer/dimension hiện tại do Pro B và agent chính tổng hợp.
- B02 `permit: Some(job)` ở image queue giữ backpressure; `FetchStickerImage` vẫn phát `permit: None` (`worker:3447–3456`) nhưng `WorkerSender::send:529` chuyển lệnh sang image queue. Pro A/Pro B thống nhất không đếm nhánh None chưa thấy caller thực thành bypass reachable.
- Không có sửa cho dư nợ A-08 WS scheme/DNS/test-env và A-09 RNG-error ở `socket.rs`/`crypto/keystore.rs` trong diff này; tiếp tục là backlog đã biết, không phải incident mới.
- `VERIFICATION.md` vẫn ghi snapshot e75e372 và một suite “đang chạy”; không dùng tài liệu đó làm test result cho f2b5e97. Gates mới thuộc agent chính, không được sao chép lời “PASS/100%” từ báo cáo bên ngoài.

## Đồng thuận với Pro B

Đã gửi A1–A4, stale clipboard, reset policy và regression logout cho Pro B phản biện trực tiếp; Pro B đã xác nhận cùng trạng thái và chấp thuận R4 ở mức P2. Hai bên giữ P2 cho retention/reset/async-disk/logout, tách runtime executor khỏi TUI render-thread. Không giữ cáo buộc cross-account reset chỉ từ `owner: _`; không giữ P1 recursion sau fix.

Pro A cũng độc lập xác nhận finding cache của Pro B: sprite strip mới ghi vào cùng `media-cache/stk_*.png` (`worker:834`), trong khi `preload_disk_sticker_inner:652–674` đọc nó như một ảnh tĩnh và gửi `StickerImageReady`; GUI `:4301` chỉ tạo texture, không có frame metadata. Cache hit khiến các kiểm tra `sticker_textures.contains_key` bỏ fetch animation (`gui/stickers:1580`). Vì vậy lần mở sau có thể hiển thị nguyên strip co lại, mất animation. **Đồng ý P2**, dùng ID của Pro B để tránh đếm đôi; test cần qua write-cache → fresh session → preload/render, không chỉ kiểm tồn tại file cache.

Không sửa source/commit/push trong lượt tái kiểm này.
