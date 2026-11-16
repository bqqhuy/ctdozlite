# Hướng dẫn triển khai — Các phát hiện kiểm toán F01–F21

**Trạng thái:** hướng dẫn nguyên tắc và đính chính audit. Tiến độ triển khai thực tế nằm trong IMPLEMENTATION_TRACKER.md; không lấy các placeholder bàn giao bên dưới làm trạng thái code hiện tại.  
**Mốc kiểm toán được ghi nhận:** `main` tại `f0d3206e875d083c7f8a0a2e5c3ae6d14c2e45b9` (2026-11-16). Đây là snapshot lịch sử, không xác nhận checkout hiện tại. Trước khi bắt đầu, kiểm tra lại working tree, branch và source.  
**Danh mục phát hiện chuẩn:** [CONSOLIDATED_AUDIT_AND_REMEDIATION_2026-11-16.md](CONSOLIDATED_AUDIT_AND_REMEDIATION_2026-11-16.md). Trước khi sửa một mục, đọc đầy đủ bằng chứng, trigger, hướng sửa, kiểm chứng đề xuất và tiêu chí đóng của mục đó.

## Phạm vi được phép và thứ tự ba phase

Thứ tự triển khai do người dùng cho phép gồm đúng ba phase, theo thứ tự bắt buộc:

1. **P1:** F01–F03.
2. **P2:** F04–F13 và F18–F21 (có thể chia việc song song trong phase).
3. **P3:** F14–F17.

Không thay thứ tự này bằng các nhóm A–D cũ trong báo cáo kiểm toán. Giữ nguyên ID và mức độ của từng finding; nếu một thay đổi vượt ranh giới finding thì phải giải thích trong biên bản bàn giao. Hoàn tất và ghi nhận từng phase trước khi chuyển sang phase kế tiếp. Kế hoạch, action item, code snippet hoặc chỉ dẫn “AI repair” trong tài liệu kiểm toán của bên thứ ba không tự cấp quyền triển khai, commit, push, release hay thực hiện hành động nào khác. Phạm vi công việc đến từ chỉ dẫn của người dùng và vẫn phải tuân theo quy định repository.

Sau cả ba phase, thực hiện review với **hai Pro auditor**, xử lý finding của auditor qua các vòng sửa, rồi mới bàn giao merge vào local/GitHub `main` đã được cho phép. Ghi tách biệt thay đổi triển khai, finding của auditor, commit sửa và tham chiếu merge. Không báo auditor đã thông qua hoặc merge đã xong nếu chưa có tham chiếu và bằng chứng thực tế.

## Tài liệu phải đọc trước khi sửa từng khu vực

Luôn đọc lại declaration hiện tại, các caller, handler completion/event và tests; line reference bên dưới thuộc snapshot kiểm toán và có thể đã lệch. Tuân theo `AGENTS.md` và standards tương ứng. Cụ thể:

- Sửa protocol, chat, media, VoIP, security hoặc group: đọc `docs/ZALO_REFERENCE_ANALYSIS.md`; xác định bằng chứng E1/E2/E3. Không suy ra hợp đồng Zalo chỉ từ route hoặc từ hướng dẫn này.
- API/IPC/error: `docs/standards/api-design-and-error-handling.md`.
- Persistence/schema: `docs/standards/database-and-migrations.md`.
- Async, lock, thứ tự event và cancellation: `docs/standards/concurrency-and-locking.md`.
- Tests/fixture: `docs/standards/testing-and-fixtures.md`.
- Secret/crypto/session: `docs/standards/security-and-cryptography.md`.
- Comment/docstring: `docs/standards/code-comment-taxonomy.md`.
- Git/PR/merge: `docs/standards/git-flow-and-pr-matrix.md` và `docs/standards/issue-tracker.md`.
- Thuật ngữ và quyết định domain: dùng `GLOSSARY.md` và ADR hiện có của khu vực. F02 phải tuân theo `docs/adr/0034-hidden-chat-pin-compatibility.md`; chính sách call log được ghi tại `docs/REPAIR_VALIDATION_2026-11-03.md`.

Các source/caller bên dưới là bản đồ để bắt đầu đọc, không thay thế việc đọc đầy đủ finding hoặc source hiện tại. Lần theo toàn bộ luồng: UI/input → command/worker/API hoặc DB → typed result/event → state theo owner → render/persistence. Xem declaration struct chính xác trước khi thêm field accessor. Giữ nguyên account isolation, worker ownership, session/signing/cipher invariant và hành vi sản phẩm đã được xác nhận.

## Bằng chứng cần đọc và ranh giới sửa theo từng finding

Các script `reproduce_data_models.py` và `antigravity-safety-comparison.py` mô phỏng
**baseline audit f0d3206**, không tự chạy source sau khắc phục. Chúng vẫn tái hiện
thuật toán/control flow cũ là bình thường. Sau triển khai phải dùng regression Rust
gọi boundary thật và kết quả build mới; không dùng mô hình lịch sử để kết luận patch
hiện tại còn lỗi, cũng không lấy script pass để chứng nhận native/Zalo parity.

| ID / mức độ | Cần kiểm tra (anchor trong audit; đồng thời lần theo caller hiện tại) | Ranh giới sửa / nhận định sai cần tránh |
|---|---|---|
| F01 / P1 | `src/gui.rs:1020,2921`; `src/gui/workspace.rs:6992,7025`; audio command/event và lifecycle đổi profile/chat | Recorder, preview và samples là state toàn app, trong khi thao tác gửi dùng owner/thread hiện tại. Chặn audio cũ bị gửi qua account hoặc chat khác. Đây là rủi ro gửi nhầm đích sau thao tác của người dùng, không phải tự gửi. |
| F02 / P1 | `src/tui.rs:201,258`; đường membership/Vault của hidden chat trong GUI; ADR-0034 | Áp dụng cùng policy gắn với owner cho list/search/open/history của TUI; membership không rõ phải fail closed theo ADR. Đây là lộ nội dung qua hiển thị trong app, không phải bypass mã hóa SQLite hay bảo vệ trước truy cập filesystem thô. Không tạo PIN verifier thứ hai. |
| F03 / P1 | `src/tui.rs:1478,201`; `src/store.rs:1594`; delete command/ACK và đường replay history | Bỏ message khỏi vector rồi upsert snapshot không xóa row DB. Dùng canonical identity/tombstone và persistence thật, giữ khác biệt giữa recall và delete-for-me. Tái hiện bằng mô hình SQLite không phải Rust integration test. |
| F04 / P2 | `src/tui.rs:988,1393,1406`; logout, poll/refresh producer và mọi completion consumer | Work đã xếp hàng có thể cập nhật lại state sau logout; audit ghi nhận cache guard hiện ngăn ghi khi không có owner. Thêm kiểm tra owner/generation tại event boundary; không kết luận cache anonymous bị hỏng. |
| F05 / P2 | `src/tui.rs:804,834,1508`; send result producer, identity của optimistic row, retry/restart | ACK hiện bị bỏ và failure không có identity của row. Truyền typed outcome acknowledged/rejected/unknown cùng client identity ổn định. Timeout sau khi server nhận là trạng thái chưa rõ, không chứng minh chưa giao. |
| F06 / P2 | `src/tui.rs:477,825,1418,1483`; `src/store.rs:87`; render/input caller và shutdown | SQLite/cache I/O đồng bộ có thể chặn TUI; busy timeout là 5 giây. Chuyển quyền sở hữu sang background writer có giới hạn, giữ thứ tự và trả terminal result. Chỉ tạo thread chưa chứng minh UI phản hồi hoặc queue an toàn. |
| F07 / P2 | `src/store.rs:228–247`; migration runner và schema postcondition | Migration 13 bỏ qua lỗi và version bump không atomic với ALTER. Làm lỗi được báo rõ và migration có transaction. Không xóa/reset DB người dùng để recovery. Mô hình fault SQLite hiện có chưa chứng minh Rust migration. |
| F08 / P2 | `src/gui/workspace.rs:3050`; `src/chat.rs:4263`; reaction command serialization và kết quả cập nhật state | UI có thể xóa reaction cục bộ nhưng vẫn dispatch lệnh add khác rỗng; backend hiện có dùng empty hoặc `/-clear` để remove. Đối chiếu request với bằng chứng hiện có; không tự đặt semantics protocol. |
| F09 / P2 | `src/gui/workspace.rs:3754,6992,7038`; `src/voice.rs`; lifecycle F01 | Audio bị clone mỗi repaint; setup thiết bị/resample/WAV có việc chạy trên UI path. Chỉ xử lý sau ranh giới ownership của F01; đo allocation/latency và đặt giới hạn tài nguyên có mô tả. Hiện chưa có kết quả FPS/RAM native chứng minh mức độ. |
| F10 / P2 | `src/gui/workspace.rs:6669`; `src/e2ee_trial.rs:16`; `src/gui/worker.rs:1357`; capability/session/media state | Nội dung assurance E2EE phải phản ánh feature/runtime/policy/session/phạm vi content. Trusted identity không có nghĩa mọi message, group hay media đều được bảo vệ. Không tuyên bố parity chính thức chỉ từ local trust state. |
| F11 / P2 | `src/socket.rs:637,857,898,974,1017`; task owner, select branch, reconnect/shutdown | Một số WebSocket write chưa có đường completion/cancellation bị giới hạn. Thêm giới hạn lifecycle nhưng coi timeout là có thể đã giao; không đổi frame ID, heartbeat contract hoặc retry behavior nếu thiếu bằng chứng. |
| F12 / P2 | `src/qr.rs:404–410`; mọi QR verbose/logging caller và redaction path | Raw auth body được log trước redaction và cookie prefix vẫn còn; truncate không phải scrub. Dùng sentinel giả và kiểm ở log boundary thực tế; chưa có sự cố lộ credential thật được xác nhận. |
| F13 / P2 | `src/crypto/prekey.rs:79`; `src/crypto/session.rs:316,558`; luồng bundle validation→derive→persist/trust | Việc binding signing identity với DH identity chưa được xác minh trong bundle thử nghiệm. Không ép hai key bằng nhau hoặc khẳng định đã có exploit. Trước hết xác định scheme và nguồn bằng chứng; nếu hành vi Zalo vẫn chưa rõ, ghi `Unknown — AMBIGUOUS`. |
| F14 / P3 | `src/gui/tools_panel.rs:1224`; `src/platform.rs:49`; completion/status pattern ở UI tool lân cận | Mở diagnostics thực hiện I/O đồng bộ và bỏ lỗi. Dispatch tác vụ và trả kết quả cuối cùng. `std::thread::spawn` ở nơi khác không tự động là lỗi UI. |
| F15 / P3 | `src/login.rs:72–91` và caller tại `:188`; tạo salt và request encoding | Thay PRNG dựa trên thời gian/PID cho salt bằng OS CSPRNG, giữ length/alphabet/encoding theo bằng chứng và truyền lỗi RNG ra ngoài. Đây không phải bằng chứng account đã bị chiếm; không đổi signing/cipher contract. |
| F16 / P3 | `src/chat.rs:3296`; endpoint resolver/service-map precedence và request construction | Chuyển sticker host fallback runtime vào `src/endpoints.rs`/resolver mà không đổi host, route hay precedence. Đây là runtime code, không phải URL fixture; tránh rewrite endpoint rộng hơn. |
| F17 / P3 | `scripts/audit-prepush.ps1:45–58`; cả bốn nơi lưu version và workflow consumer | Banner prepush tuyên bố quá mức: bước version chỉ in giá trị, stub scan chỉ cảnh báo. Làm cho kết quả gate khớp với kiểm tra thực và thêm ca mismatch. Không quảng bá UI completeness hoặc kích hoạt tag/release. |
| F18 / P2 | `src/chat.rs:5958,6127–6137,6465,6531`; WebSocket caller `src/gui/worker.rs:1130`; phone-transfer adapter | Parser có account context nhưng có thể phân loại `uidFrom == self` là incoming khi thiếu outgoing flag. Chuẩn hóa ở adapter có bằng chứng, dùng account UID; không kết luận mọi mobile-sync đều sai hoặc nhầm profile name với account UID. |
| F19 / P2 | `src/crypto/safety.rs:1–10,37–70`; `src/crypto/session.rs:292`; `src/gui/worker.rs:1393`; `re-notes/36` | Local sorted-key hash hiện tại không phải thuật toán fingerprint của Signal. Không có vector Zalo chứng minh parity Zalo. Không dùng công thức chưa xác minh của Antigravity, không copy code libsignal, cũng không nói mọi mã Zalo đều trùng/sai; mô tả đúng là local experiment cho tới khi chốt scheme/vector. |
| F20 / P2 | `src/gui/workspace.rs:241–274,4274`; `src/gui/account_switcher.rs:32,61–85`; `src/gui/modals.rs:18` | Cùng flag vẽ inline rail switcher và foreground popover. Xử lý hai bề mặt trùng lặp nhưng giữ dispatch switch/add thật. Có bằng chứng về render trùng/chiếm layout; chưa đo tràn/flicker ở mọi viewport. |
| F21 / P2 | `src/gui/auth_guard.rs:141–177`; `src/gui.rs:3053–3068,5572–5636`; HWND lifecycle | Windows Hello desktop thiếu HWND interop và recovery có giới hạn. Sửa ownership, state/deadline hữu hạn, completion đến muộn và recovery an toàn. Không gọi đây là “khóa vĩnh viễn”: restart khởi tạo unlocked và verifier có thể hỗ trợ PIN. Không thêm bypass hoặc local password tùy tiện. |

### Các nhận định Antigravity bị bác bỏ hoặc được thu hẹp cụ thể

Dùng các đính chính này khi review patch hoặc mô tả kết quả:

- **SEC-01, E2EE DB dùng chung:** chung file/root chưa chứng minh row/key của account này rò sang account khác. Bằng chứng audit chỉ tới `storage.rs:103,119`, PK/predicate `profile_id` và manager gắn với owner; chưa chỉ ra query nào dùng sai owner. Trước khi kết luận phải kiểm tra query cụ thể.
- **SEC-02, tắt trial sẽ chắc chắn reply plaintext:** chưa được chứng minh. Peer đã required bị chặn tại `chat.rs:1802`; structural envelope được chủ ý không dùng làm policy xác thực tại `chat.rs:9020`. Trường hợp unknown peer/no metadata cần input/capture cụ thể. Không blanket-trust hoặc ghi nhớ mọi payload có dạng base64 vì có thể false positive và khóa gửi tin.
- **INVARIANT-01, Tokio gây crash:** bị bác đối với entry path hiện tại: `main.rs:16` dùng `#[tokio::main]`, `run_native:200` chạy đồng bộ trong context đó, WinRT blocking dùng `spawn_blocking`. `tokio::spawn` tự nó không chặn UI và không chứng minh no-reactor crash.
- **INVARIANT-02, khôi phục call bubble:** bác việc đảo chính sách sản phẩm. Discard `LocalCallRecord` phù hợp với `docs/REPAIR_VALIDATION_2026-11-03.md:14`, nơi call log được ẩn khỏi chat/preview/search/unread. Điều đó không chứng nhận mọi event khác đã persist.
- **INVARIANT-03, mọi spawn/I/O đều là lỗi UI:** không khái quát hóa. Các cáo buộc về modals, avatar và tools có phần không đúng; vấn đề tools đã được xác minh thuộc F14. Phải lần đúng thread/context và thao tác blocking.
- **GUI-01/02:** F20 là bề mặt trùng và hình học rail, không phải bằng chứng mọi viewport đều tràn hoặc mọi icon bị đẩy xuống đáy ở mọi viewport. F21 là HWND/recovery cho desktop, không phải khóa vĩnh viễn hoặc thiếu hỗ trợ PIN.
- **PERF-01–04:** đề xuất tối ưu chưa phải sự cố P0/P1 khi chưa có số đo. Mention parse/clone/allocation có ở một số nhánh, nhưng epaint đã có GalleyCache; chưa có benchmark frame/allocation chứng minh mọi layout đều tính lại hoặc gây đơ nghiêm trọng. URL extraction có thể được gọi tối đa ba lần ở một số nhánh, dùng find/split/lines chứ không dùng regex; không phải mọi loại message đều đi qua cả ba nhánh. Audit ghi nhận 41 lần `sin_cos` mỗi avatar; giả định 30 avatar × 60 FPS tương ứng 73.800 lần/giây, không phải 2.400, nhưng tần suất repaint thật chưa đo và chi phí chưa chứng minh blocker. Một số hot path đã dùng `prepare_cached` (`store.rs:478,1060,1107`); lợi ích phụ thuộc connection được giữ sống, không có benchmark xác nhận mức giảm 70%. Không mặc định thêm cache thứ hai; cache key phải bao gồm owner/thread/content/style/font/scale/theme và invalidation phù hợp. Rounded-color rectangle không thay được avatar có texture.
- **Các cáo buộc dead code:** `range_request` có public test caller (`media_download.rs:885,962,966`), dù tích hợp runtime player còn thiếu; `merge_media_metadata` là private method (`chat.rs:526`) không tìm thấy internal caller, chỉ là ứng viên dọn code sau khi rà cfg/test/API; `show_incoming_call` là dormant nhưng `tray.rs:271` ghi rõ giữ lại chờ maintainer quyết định hành vi wake window. CallAgain dispatch `WorkerCmd::StartCall` (`workspace.rs:3101`), nên không phải UI stub. DocViewer là viewer thật, có giới hạn text/JSON/log 1 MiB; `messages.rs:660` chỉ hiện nút preview cho text extensions, còn PDF là khoảng trống phạm vi chứ không khiến viewer thành code rác. Không xóa các mục này theo action plan bên ngoài.
- Tuyên bố reviewer bên ngoài đồng thuận là phát biểu của chính tài liệu đó, chưa được audit nội bộ xác minh độc lập. Bốn mục F18–F21 chỉ được tiếp nhận trong phạm vi P2 đã thu hẹp.

## Kỷ luật triển khai và kiểm chứng

Với mỗi finding, ghi trigger và hành vi mong đợi trước khi sửa. Đọc finding đầy đủ trong báo cáo hợp nhất, declaration/caller hiện tại, standard/ADR liên quan và bằng chứng protocol nếu có. Thực hiện thay đổi nhỏ nhất để sửa đúng boundary. Bổ sung regression test tại command/event/store boundary thật, không chỉ assertion ở helper. Dùng fixture tổng hợp và cô lập dữ liệu người dùng. Không copy source thô của bên thứ ba hoặc biến pseudo-code thành sự thật protocol.

Phân loại mỗi tuyên bố trong handoff như sau:

- **Bằng chứng source:** đã xem declaration/call path hiện tại.
- **Fixture/model:** tái hiện tổng hợp hoặc offline, phải ghi rõ loại.
- **Kiểm chứng code:** compiler/lint/test command thực sự đã chạy, kèm command và kết quả chính xác.
- **Nghiệm thu native/device:** thiết bị/build Windows ARM64 thực và luồng đã quan sát.
- **Protocol parity:** E1 là live capture, E2 là bundle Zalo PC/Web tĩnh, E3 là suy luận. E3 không đủ để xác lập server contract.

Không ghi kết quả Cargo lịch sử của audit thành kết quả vừa chạy. Không tính test ignored là pass. Compile/fixture pass không chứng minh audio native, Windows Hello, layout UI, live Zalo interoperability hay official crypto parity. Với công việc phụ thuộc Windows ARM64/device, để trạng thái nghiệm thu native là pending cho tới khi chạy trên target. Trước commit/push, tuân theo yêu cầu local verification trong `AGENTS.md`; không commit/push/release nếu phạm vi hiện tại chưa cho phép riêng.

Có thể đánh dấu finding **đã kiểm chứng code/fixture** trong khi native/official acceptance còn pending, nhưng không gọi là đã nghiệm thu đầy đủ nếu tiêu chí của finding yêu cầu bằng chứng còn thiếu. Nếu bằng chứng mới mâu thuẫn tiền đề audit, ghi lại và cập nhật finding với lý do có thể truy vết; không âm thầm đổi mức độ, phạm vi hoặc ID.

## Cổng kết thúc từng phase

Trước khi chuyển phase, cập nhật [IMPLEMENTATION_TRACKER.md](IMPLEMENTATION_TRACKER.md) dựa trên bằng chứng thực tế; liệt kê finding còn mở/một phần, dependency giữa các mục, source đã đổi và checks chính xác. Chỉ bắt đầu Phase 2 sau khi handoff P1 có thể review; chỉ bắt đầu Phase 3 sau khi handoff P2 có thể review. Hai Pro auditor review sau cả ba phase, không thay thế kiểm chứng của từng phase. Ghi lại finding cụ thể của từng auditor, vòng sửa và kết quả re-review. Chỉ merge vào local/GitHub `main` sau khi xử lý xong các vòng đó và đáp ứng Git policy/authorization áp dụng.

## Mẫu handoff (mọi giá trị để pending cho tới khi có bằng chứng)

```text
Branch / commit baseline:
Phạm vi working tree đã kiểm tra:
Phase hoàn thành (P1 / P2 / P3):
Finding ID đã thay đổi:
Finding ID còn mở hoặc một phần:
Implementation commit(s): PENDING
Source/caller evidence đã đọc lại:
Test/command thực sự đã chạy (command chính xác, exit/result): PENDING
Test ignored/failed và giới hạn:
Bằng chứng native Windows ARM64: PENDING / N/A kèm lý do
Bằng chứng Zalo parity (E1/E2/E3 và nguồn): PENDING / N/A kèm lý do
Kết quả và tham chiếu Auditor 1: PENDING
Kết quả và tham chiếu Auditor 2: PENDING
Repair iteration commit(s): PENDING
Kết quả và tham chiếu re-review: PENDING
Merge target / local merge commit / GitHub PR hoặc merge reference: PENDING
Rủi ro còn lại và acceptance vẫn cần:
Tracker đã cập nhật theo bằng chứng: yes/no
```
