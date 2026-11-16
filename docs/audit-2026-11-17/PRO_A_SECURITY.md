# Pro A — Kiểm toán bảo mật, riêng tư và xử lý lỗi

Ngày: 17/11/2026. Baseline: `main`, `e75e3723c54e42191a2a81ee507f3e0f496c50fc`.

**Kết luận: chưa chấp thuận tuyên bố hoàn tất khắc phục hoặc sẵn sàng nghiệm thu.** Các sửa cũ có giá trị, nhưng vẫn còn lỗi parser, vòng đời dữ liệu riêng tư và kết quả thao tác; nhánh sticker mới đưa vào lỗi kích thước texture đã được Pro B tái hiện an toàn. Không đồng nhất kiểm tra compiler/unit với nghiệm thu Windows ARM64 hoặc tương thích Zalo.

## Phạm vi và bằng chứng

Đọc `AGENTS.md`, skill Karpathy và Graphify; không có `graphify-out/graph.json` nên dùng tìm kiếm/đọc nguồn trực tiếp. Đọc chuẩn security, API/error, concurrency, `ZALO_REFERENCE_ANALYSIS.md`, tracker, handover, báo cáo hợp nhất F01–F21 và consensus `bd44cc1`. Kiểm tra session/logout/auth, E2EE/wire, giới hạn dữ liệu, runtime, media/clipboard và đối chiếu các sửa sau audit.

Lượt Pro A này chỉ phân tích tĩnh và viết báo cáo. Không sửa source/tests/config; không chạy Cargo, không mở app với dữ liệu thật, không đọc credential/DB/capture riêng tư, không gọi Zalo, không tạo payload gây crash. Kết quả build/test mới do agent chính tổng hợp riêng. Probe sticker là sản phẩm của Pro B; Pro A đọc probe và độc lập xác minh đường code, không tự nhận đã chạy probe. Số dòng dưới thuộc baseline trên.

## Phát hiện ưu tiên

### A1 — P1: Parser protobuf thiếu giới hạn độ sâu trước khi gọi đệ quy

- **Nguồn:** `src/crypto/wire.rs:453–463`, đặc biệt lời gọi đệ quy dòng 460 và kiểm tra loại sau lời gọi dòng 463. `src/e2ee_integration.rs:216–217`; `src/chat.rs:9009–9022`.
- **Điều kiện:** dữ liệu tin có metadata E2EE được đưa vào bước kiểm tra cấu trúc wire. Nhánh nested-message vẫn gọi cùng parser trước khi loại trừ nested PreKey. Đây là đường xử lý cục bộ tồn tại cả trong build mặc định: guard hợp đồng thử nghiệm đến sau bước `is_e2ee_wire_payload`.
- **Ảnh hưởng:** độ sâu lồng nhau không được chặn tại boundary; tăng nguy cơ cạn stack và mất khả dụng của tiến trình. Giới hạn HTTP 8 MiB/WebSocket 16 MiB chỉ chặn tổng byte, không thay thế giới hạn recursion. Không khẳng định đã tái hiện crash, có thể gửi payload qua server Zalo, hoặc replay tự động.
- **Đối chiếu cũ:** C-01 đã sửa overflow/offset và ngăn plaintext thông thường đi vào parser. Không bác bỏ sửa đó; đây là lớp thiếu sót riêng của cùng boundary parser.
- **Sửa:** tách parser Whisper khỏi parser PreKey; trường nested-message chỉ được parse bằng parser Whisper. Nếu giữ hàm chung, thêm ngân sách depth trước lời gọi; áp dụng ngân sách size/field phù hợp.
- **Kiểm chứng đề xuất:** fixture nhỏ, độ sâu hữu hạn, đúng/sai loại nested; phải trả lỗi sớm tại boundary, không cần fixture crash. Giữ kiểm tra plaintext gate và roundtrip hợp lệ. Official parity: **Unknown — AMBIGUOUS**; giới hạn parser là hardening nội bộ.
- **Phản biện:** Pro B xác nhận ưu tiên P1 cho availability; chúng tôi loại bỏ diễn đạt “remote exploit đã chứng minh”.

### A2 — P2: M-05 chưa hoàn tất, ảnh clipboard/chụp màn hình vẫn lưu plaintext ngoài scope tài khoản

- **Nguồn:** `src/gui/messages.rs:214–220` ghi `clipboard_<ms>.png` vào `storage::cache_dir`; `src/storage.rs:211–212` trỏ tới `root/media-cache`, trong khi account temp ở dòng 164–165. `src/gui/workspace.rs:3388` hủy bằng cách bỏ reference. `src/gui.rs:2483–2493` bỏ event khác owner/thread không dọn file. `src/profiles.rs:167–173` chỉ dọn account temp lúc logout.
- **Điều kiện:** dán ảnh hoặc chụp màn hình, sau đó hủy hoặc logout; file tạm đã được tạo nhưng không thuộc cây được cleanup. Không cần gửi ảnh lên server.
- **Ảnh hưởng:** bản sao nội dung riêng tư vẫn nằm trên đĩa sau khi người dùng bỏ thao tác/logout. Đây là retention cục bộ, không phải bằng chứng tài khoản khác tự nhìn thấy ảnh trong UI, cũng không khẳng định người dùng Windows khác có quyền đọc.
- **Đối chiếu cũ:** M-05 gốc nêu rõ cả WAV và clipboard. Voice đã chuyển vào account temp ở `src/gui/worker.rs:3671`, nhưng đó chỉ là sửa một phần. `clear_account_temp` chỉ có caller production tại logout; `prune_old_temp_files` chưa có caller production. Không coi test helper cleanup là chứng minh lifecycle đã nối đủ.
- **Sửa:** account-scoped temporary lease cho clipboard/capture/voice; dọn sau gửi, hủy, stale result, logout/switch và startup orphan sweep. Không xóa file gốc do người dùng chọn.
- **Kiểm chứng đề xuất:** ảnh tổng hợp trong data root temp; kiểm tra create/cancel/send/error/switch/logout/restart ở caller thật; kiểm tra file tài khoản B và file nguồn vẫn nguyên vẹn.
- **Phản biện:** Pro B đồng ý giữ đây là **M-05 còn một phần**, không gọi là regression mới.

### A3 — P2: Reset E2EE báo thành công trước kết quả và chưa phục hồi đúng chính sách

- **Nguồn:** `src/gui/workspace.rs:5881–5903`: nút chỉ hiện khi peer nằm trong `trial_peers`, gửi command rồi lập tức xóa peer và toast thành công. `src/gui/worker.rs:1522–1529`: bỏ qua lỗi DB và JoinError, không phát terminal event. `src/e2ee_integration.rs:102–113`: chỉ xóa marker `e2ee_required`. `src/crypto/session.rs:226–233`: requirement còn đúng nếu ratchet session tồn tại.
- **Điều kiện:** channel lỗi, DB không ghi được, account đã đổi, hoặc peer có ratchet session; UI vẫn tuyên bố đã gỡ. Sau restart, `trial_peers` khởi tạo rỗng (`src/gui.rs:1379`), chỉ được thêm ở kết quả Prepare (`:2304`), nên recovery của marker cũ không được hydrate từ policy bền vững.
- **Ảnh hưởng:** false success, nút biến mất trong khi điều kiện chặn gửi còn tồn tại. Luồng cũng thiếu bước xác nhận chuyển sang nhắn thường mà remediation M-01 đã yêu cầu.
- **Giới hạn:** không khẳng định rò plaintext khi DB lỗi. `check_thread_e2ee` vẫn trả lỗi và boundary gửi thật phải giữ fail-closed. Owner check trong worker có thật, không suy cross-account mutation.
- **Sửa:** command/result có owner/generation; UI giữ pending và chỉ đổi trạng thái sau thành công; hydrate trạng thái policy; xác nhận có giải thích hậu quả. Định nghĩa rõ marker reset khác ratchet reset, không âm thầm xóa key/session để làm cho nút có vẻ hoạt động.
- **Kiểm chứng đề xuất:** channel closed, DB failure, stale owner, marker-only, existing ratchet và restart. Kiểm tra terminal event/UI state và next-send policy; không gửi thật.
- **Phản biện:** Pro B và agent chính đều xác nhận P2. Việc gỡ marker đơn giản không đủ chứng minh đã khắc phục toàn bộ M-01.

### A4 — P2: M-06 mới sửa một phần, SQLite đồng bộ còn chạy trên executor

- **Nguồn:** runtime hai thread `src/gui/worker.rs:1037`; `SyncHistory` gọi `history_sync::download` từ `tokio::spawn` ở `:2743–2746`. Hàm async `src/history_sync.rs:215` trực tiếp mở SQLite `:248`, đọc checkpoint `:263`, persist `:302`, ghi checkpoint `:308` và filter `:315`.
- **Điều kiện:** tải lịch sử khi DB/DPAPI/đĩa chậm hoặc khóa SQLite. Công việc đồng bộ chiếm thread executor trong khoảng đó.
- **Ảnh hưởng:** ảnh hưởng responsiveness/liveness các task dùng chung runtime; không có đo mới để khẳng định 45 giây timeout hoặc phần trăm chậm. Đây khác F06 là I/O trên thread render TUI.
- **Đối chiếu cũ:** offload `save_to_file` ở một số nhánh là đúng nhưng không giải quyết toàn bộ phạm vi M-06 đã nêu. Sticker mới còn gọi decode đồng bộ tại worker `:758–809`; Pro B sở hữu finding riêng về regression sticker, không cộng hai lần cùng root cause trong tổng hợp.
- **Sửa:** store actor hoặc các đơn vị `spawn_blocking` có ownership/cancellation rõ ràng; giữ network await ở async và thứ tự persist-before-checkpoint.
- **Kiểm chứng đề xuất:** fixture DB riêng có delay/lock được kiểm soát, đo tiến độ timer/network task và cancellation; kiểm chứng checkpoint không vượt dữ liệu đã persist. Chưa chạy benchmark ở lượt này.

## Phản biện độc lập kết quả Pro B

| Kết quả | Kết luận Pro A | Cơ sở/giới hạn |
|---|---|---|
| Bundled GIF tạo texture quá rộng | **Đồng ý P1** | `src/bundled_stickers.rs:158–186` ghép ngang full-size; `src/gui.rs:2128–2142` chỉ chặn byte rồi gọi `load_texture`. Probe B dùng asset #02 và egui thật, 12800×560 vượt max 8192. Không cần nội dung độc hại. Pro A xác nhận source, không tự chạy probe. |
| Sprite event bỏ permit, queue giữ decoded pixels | **Đồng ý P2** | job nhận `worker:1086`; `StickerSpriteReady:1093` không mang permit; `PreviewImageReady:1112` có permit; channel không giới hạn (`gui.rs:1273`). Budget texture chỉ tính khi UI nhận. Với pack 15 ảnh, không gọi số lượng asset “vô hạn”; vấn đề là không còn backpressure/budget cho output chờ xử lý. |
| Decode GIF/ảnh trực tiếp trong async | **Đồng ý P2**, gộp với runtime blocking khi tổng hợp | `worker:764/768/793/805/809`, runtime hai thread. Không nhận KPI/freeze duration chưa đo. |
| Sticker bỏ private disk cache cũ | **Đồng ý P2 về regression offline/cache** | Nhánh mới `worker:1090–1096` đi thẳng fetch; nhánh cũ đọc cache `:928–938`, ghi `:974`. Public sticker pixel không tự là leak qua tài khoản. |

## Đối chiếu tiến độ cũ trong miền Pro A

“Sửa ở source” bên dưới không phải kết quả test mới; agent chính tổng hợp gates thực chạy. Ma trận toàn bộ F01–F21 do agent chính/Pro B ghép với phần này, không suy tất cả 21 mục đã được Pro A audit từng dòng.

| Mục | Trạng thái kiểm tra hiện tại | Bằng chứng/phần còn lại |
|---|---|---|
| F10 assurance UI | Sửa cũ còn hiện hữu; recovery mới chưa đúng | `e2ee_security_summary`, `workspace:5870`; A3 là thiếu sót riêng. |
| F11 bounded WS writes | Sửa ở source còn hiện hữu | `socket:74–96` và callers heartbeat/pull/pong; native/network soak chưa chạy. |
| F12 + L-02 redaction | Sửa ở source còn hiện hữu | QR verbose chỉ metadata `qr:386–388`; tokenizer UTF-8 `session:1345+`; không đọc log thật. |
| F13 / F19 identity & fingerprint | Đóng local/trial theo phạm vi hẹp | XEdDSA gắn DH identity `crypto/session:559–574`; fingerprint 5200 vòng và vector `crypto/safety:12,130`. Zalo parity vẫn Unknown — AMBIGUOUS. |
| F15 salt | Sửa ở source còn hiện hữu | `login:77–82`, `random:3+`: OS RNG, propagate failure, rejection sampling. |
| F21 + M-02 Hello/recovery | Sửa thêm code có thật; native vẫn pending | `gui.rs:1539` initial lock, `:5932–5948` nút logout; `auth_guard:167+` HWND/deadline/admission. Không giữ kết luận cũ “không có đường logout” sau sửa mới. |
| C-01 overflow | Sửa cụ thể còn hiện hữu; boundary parser chưa hoàn chỉnh | checked offset/varint `crypto/wire:119–161`, plaintext gate `chat:9015`; A1 là thiếu depth cap. |
| H-01 attachment launch | Sửa ở source còn hiện hữu | `platform:107,150–157,191–202`: allowlist/MOTW/Explorer fallback; Windows ARM64 shell acceptance pending. |
| M-01 E2EE recovery | **Một phần** | Default metadata không tự ghi requirement ngoài feature; A3 cho thấy recovery chưa end-to-end. |
| M-03 / L-03 logout | Routine chung và terminal failure có thật | `profiles:167`, `worker:2192–2208`, TUI caller `tui:1885`; remote revocation chưa chứng minh. |
| M-05 temp privacy | **Một phần** | Voice đổi scope, clipboard vẫn global, lifecycle sweep chưa nối đủ (A2). |
| M-06 async I/O | **Một phần** | Targeted session-save offload không bao trùm history SQLite (A4). |
| M-07 incoming link preview | Sửa tại boundary có thật | `chat:2994–2999` chỉ scrape Outgoing; UA `chat:2825`; không live test. |

Các trạng thái “Một phần” F02/F03/F04/F05/F06/F08/F09/F20 trong tracker không tự nâng thành hoàn tất vì chưa có integration/race/benchmark/native tương ứng. Bản báo cáo hợp nhất cũ ở cuối ghi tất cả F01–F21 mở là ảnh chụp trước sửa; không dùng dòng lịch sử đó để phủ nhận các commit khắc phục sau này.

## Dư nợ bảo mật kế thừa cần theo dõi

- **A-08 transport hardening, chưa đóng:** `socket.rs:164,184` cho biến môi trường test có hiệu lực ngoài test; DNS lookup lỗi đi qua `:193–202`; `run_once:749–766` chưa bắt buộc scheme `wss` trước gắn credential. Đây là thiếu kiểm tra phòng thủ đã có trong backlog cũ. Muốn đánh giá tác động cần xét nguồn endpoint/service-map và điều kiện cấu hình; không chứng minh credential đã rò, DNS rebinding thành công hay server chính thức trả `ws`. Nên fail-closed scheme/DNS và compile-gate ngoại lệ test, thêm fixture không chứa cookie thật.
- **A-09 crypto hygiene, chưa đóng:** `crypto/keystore.rs:121–123` bỏ qua lỗi RNG cho nonce XEdDSA. Không suy private-key compromise từ riêng đoạn này: nonce hash còn phụ thuộc secret và message. Sửa bằng trả lỗi rõ từ signing API và fixture injected RNG failure; zeroization/encoding contract tiếp tục cần review riêng.
- Các giới hạn đang có là điểm tốt: HTTP/inflate 8 MiB, ảnh input 16 MiB và dimension/allocation (`limits.rs:9–50`), WS frame/message 16 MiB (`socket:155,772–775`), skipped ratchet keys 1000/2000 (`crypto/ratchet:43–46`), account path hash (`storage:157–160`), session DPAPI/atomic write và clone revocation (`session:558–610,635–659`). Các kiểm soát này không thay thế kiểm tra depth hoặc vòng đời file.

## Thứ tự xử lý đề nghị

1. Sửa texture limit và parser depth P1; regression nhỏ tại boundary thật.
2. Hoàn tất M-05, reset E2EE có kết quả/xác nhận và scope policy đúng; giữ fail-closed khi lỗi DB.
3. Khôi phục backpressure/cache/offload sticker và xử lý phần M-06 còn lại; đo sau sửa.
4. Nghiệm thu Windows ARM64, Hello/micro/renderer và gates native; chỉ nâng parity E2EE bằng bằng chứng chính thức đủ mức.

Không có commit/push/release hoặc source fix trong lượt Pro A này.
