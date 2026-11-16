# Audit độc lập: dữ liệu, logic, đồng thời và TUI

Baseline: `f0d3206e875d083c7f8a0a2e5c3ae6d14c2e45b9`, nhánh main. Chỉ đọc source; không gọi API thật, không dùng dữ liệu người dùng, không chạy cargo trong agent này. Các kết luận dưới đây dựa trên đường đi mã xác định, chưa chạy native GUI/TUI.

## Findings đề nghị đưa vào đồng thuận

### D1 — P1: Xóa tin trong TUI không xóa SQLite, tin xuất hiện lại

- `src/tui.rs:1478`: DeleteDone chỉ retain bỏ tin khỏi vector, rồi persist_thread_by_id. `src/tui.rs:477` gọi persist_threads.
- `src/store.rs:1594`: persist_snapshot chỉ upsert các hàng có mặt, không xóa hàng vắng mặt; không tạo tombstone deleted_messages.
- `src/tui.rs:201`: khởi động TUI hydrate lại từ SQLite. Do hàng cũ còn nguyên, tin đã báo xóa xuất hiện lại. Poll đang chạy cũng có thể merge lại tin cũ.
- `src/chat.rs:4380`: API delete_message_for_me chỉ xác nhận mutation server; không tự sửa SQLite. GUI có bước delete_chat_message riêng, nên không thể coi API helper đã xử lý.
- Khắc phục đề nghị: sau ACK, chạy delete_chat_message trong worker/blocking task, có marker và transaction; phát sự kiện hoàn tất sau khi lưu. Regression: xóa → mở lại DB/TUI → replay lịch sử không hồi sinh.
- Security agent đã đọc độc lập và đồng ý finding.

### D2 — P1: TUI bỏ qua khóa PIN hội thoại ẩn

- `src/main.rs:314` nạp cache và SQLite, `src/main.rs:350` truyền toàn bộ vào TUI.
- `src/tui.rs:201` tiếp tục hydrate lịch sử; không có tham chiếu hidden/enforced trong toàn bộ TUI, không lọc membership và không có bước unlock trước hiển thị.
- ADR `docs/adr/0034-hidden-chat-pin-compatibility.md:13` yêu cầu gate UI/search/export; dòng 22-24 yêu cầu restart offline vẫn giữ khóa hidden membership. Không có miễn trừ TUI.
- Trigger: người dùng đã có lịch sử hội thoại ẩn trên máy, đóng GUI rồi mở chế độ --tui: history được đọc qua đường không gate PIN.
- Đây là bypass lớp riêng tư UI; không gọi là phá mã hóa SQLite (repo công bố SQLite không mã hóa).
- Đề nghị dùng cùng owner-bound Vault/membership cho mọi UI mode và fail closed cho hidden khi chưa có verifier. Security agent xác nhận phạm vi và đồng ý.

### D3 — P2: I/O SQLite và cache chặn vòng render TUI

- `src/tui.rs:477-491` persist_snapshot chạy trực tiếp SQLite; `src/tui.rs:825-826` gửi tin gọi persistence và save_threads_cache đồng bộ; HistoryLoaded/DeleteDone làm tương tự tại 1418/1483.
- `src/tui.rs:1547` vòng render gọi check_background_events và handle_key trên cùng thread. DB busy_timeout tại `src/store.rs:87` là 5 giây; đồng thời persist_async có thể giữ writer lock.
- Vi phạm INV-3 và ownership standard: worker owns SQLite. Khi disk chậm hoặc writer cạnh tranh, thao tác gửi/xóa/history có thể đứng UI nhiều giây; mọi lỗi ghi bị bỏ qua.
- Đề nghị chuyển writes/cache vào single-writer background queue, trả terminal event có lỗi; không giữ connection SQLite trên TUI. Kiểm tra bằng synthetic writer lock/temp DB.

### D4 — P2: TUI mất ACK và định danh lỗi gửi

- `src/tui.rs:804-825` tạo và lưu optimistic local message ngay, không outgoing_job và không trạng thái lỗi riêng.
- Các nhánh send tại `src/tui.rs:834-852` bỏ payload ACK bằng map(|_|()); BgEvent::MessageSent chỉ thread/text, SendFailed chỉ chuỗi lỗi.
- `src/tui.rs:1508` chỉ đổi status khi thất bại; không thể xác định hàng nào để đánh failed/retry. Nếu server từ chối, hàng optimistic tồn tại trong DB và xuất hiện lại sau restart dù chưa giao.
- Khắc phục: mang client ID + ACK/typed outcome qua event, cập nhật delivery cho đúng hàng, lưu retry payload phù hợp; giữ unknown outcome khác rejected. Không suy luận lỗi HTTP timeout chắc chắn chưa giao.

### D5 — P2: Migration 13 có thể công bố version mới dù ALTER thất bại

- `src/store.rs:228-247` nuốt lỗi introspection, ALTER TABLE và PRAGMA user_version. ALTER và bump version không cùng transaction.
- Với DB schema12 thiếu last_read_msg_id và ALTER bị lỗi (authorizer denial/I/O/schema conflict), bước bump vẫn có thể thành công; open trả Ok nhưng các truy vấn thread đọc cột không tồn tại. Khi mở lại schema13, migration bị bỏ qua.
- Đề nghị dùng transaction, kiểm tra schema strict, propagate errors và chỉ bump khi thành công. Đây là nhánh lỗi migration xác định từ source, chưa tái hiện bằng rusqlite; không tuyên bố đã gặp trên DB thật.

### D6 — P2: Sự kiện TUI xếp hàng trước logout có thể hồi sinh dữ liệu riêng tư

- `src/tui.rs:988-995` logout xóa state, nhưng bg_rx/bg_tx vẫn giữ và các event không có owner/generation.
- `src/tui.rs:1393` PollDone áp dụng không kiểm tra login/generation. `src/tui.rs:1293` merge polled threads vào state hiện tại.
- Trigger: poll hoàn tất đã xếp PollDone vào channel; người dùng xác nhận logout trước vòng check_background_events kế tiếp. Vòng sau nạp lại threads cũ và render sau thông báo đăng xuất.
- Session revocation không thu hồi event đã xếp; save_threads_cache có guard empty owner nên không khẳng định cache của tài khoản cũ bị ghi. persist_async có thể nhận session rỗng nhưng finding chính là UI exposure.
- Đề nghị owner/runtime ID trong mọi event, discard stale events, abort/drain tasks khi logout. Regression inject queued event rồi logout.

## Thuật toán, hiệu năng và code thừa

- `src/tui.rs:1340-1345` mỗi thread incoming lại position tìm trong self.threads: O(T²) khi poll chứa T hội thoại; `apply_refresh:1373` cùng kiểu. Đề nghị đo fixture 100/1000/10000 thread, dùng HashMap<thread_id,index> trong mỗi batch để đạt O(T+M); giữ nguyên identity/order semantics. Chưa đo thời gian nên không hứa tỷ lệ tăng tốc.
- `persist_async:502` clone toàn bộ thread/messages mỗi poll rồi persist toàn bộ snapshot, khác GUI đã có delta refresh. Đề nghị chỉ ghi delta sau benchmark và vẫn giữ account/deletion/recall invariants.
- Không đề nghị xóa public API chỉ vì ít call sites: feature flags, tests, native Windows và library consumers có thể dùng. Không có ứng viên xóa an toàn được chứng minh trong mảng này.
- Đã kiểm tra upload checkpoint: contiguous_completed_prefix dùng binary search (`src/upload.rs:160`), checkpoint tuple prefix/sparse; không báo nhầm O(N²) từ comment O(N) cũ. Comment là lệch tài liệu nhỏ, không phải lỗi thuật toán hiện tại.

## Phản biện chéo đã hoàn tất

- Đồng ý GUI voice P1: recorder/samples không được clear tại LoggedOut/ProfileSwitched (`src/gui.rs:2918-3055`), `src/gui/workspace.rs:7025` dùng bản ghi cũ với thread/worker hiện tại; cần owner binding và cancel recording/preview khi account đổi.
- Đồng ý reaction P2: `src/gui/workspace.rs:3050-3073` clear locally nhưng gửi reaction không rỗng; `src/chat.rs:4263` chỉ empty hoặc /-clear mới có rType=-1.
- Đồng ý socket P2: một số ws.send trong select await không timeout (`src/socket.rs:857,898`), có thể giữ read/liveness loop khi transport nghẽn; không có outer whole-connection timeout bảo vệ.
- Đồng ý crypto P2 có giới hạn experimental: `src/crypto/session.rs:558` tin signingIdentityKey tách rời identity_key, trong khi X3DH/trust dùng identity_key tại316/345; chưa có binding được kiểm tra. Không gọi đây là live exploit đã xác nhận.

Các mục trên là ý kiến audit độc lập và peer review gửi parent; số/nhãn cuối do báo cáo đồng thuận hợp nhất quyết định.

Coverage bổ sung: đã đọc archive.rs (parse/list/versioned mutation readback), group_metadata.rs (membership-version display cache), group_state.rs (runtime/owner-bound permission cache và invalidation); không phát hiện lỗi đủ chắc để thêm finding. group_entry/group_content chỉ rà surface/đường gọi, không chứng nhận audit mọi nhánh. VoIP/media/voice chủ yếu rà ownership/loop/cancellation, không kiểm chứng thiết bị hoặc giao thức live.

## Tái hiện offline và đồng thuận cuối

`reproduce_data_models.py` chạy thành công; `data-model-results.json` lưu 2 probes SQLite synthetic: D1 remaining upsert không xóa hàng cũ; D5 fault-injected ALTER thất bại nhưng user_version vẫn lên13. Đây là mô hình control flow, không phải chạy MessageStore Rust, không phải dữ liệu thật.

Security và GUI quality đã đọc và độc lập ACCEPT D1-D6 cùng severity/điều kiện nêu trên. Agent này đồng ý thêm GUI03 P2 voice setup/copy samples/resampling chặn UI và GUI05 P3 diagnostics I/O trên UI (`src/gui/tools_panel.rs:1225-1226`). Đồng ý QR raw verbose P2 (nếu response có secret), LCG salt P3, drawer E2EE assurance P2, prepush assurance gap P3. Không còn bất đồng cần giữ với các đề xuất được gửi phản biện ở vòng hiện tại.
