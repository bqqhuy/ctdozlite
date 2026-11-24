# .agent/PLAN.md - Kế hoạch Hành động Zalo Parity Hợp nhất (Phase 1 -> Phase 6) - 100% HOÀN THÀNH

Tài liệu tổng thể 6 Phases: `docs/research/2026-11-24-zalo-parity-unified-master-plan.md`
Biên bản nghiệm thu & Audit: `.agent/REVIEW_LOG.md`
Danh sách kiểm tra & Nhiệm vụ: `.agent/TASKS.md`
Chuẩn mực kỹ thuật: `AGENTS.md` (Karpathy-inspired coding discipline, Parity Evidence, Non-Negotiables)

---

## Trạng thái Hoàn thành Toàn diện 6 Phase

| Phase | Trọng số | Nội dung cốt lõi | Trạng thái | Reviewer Consensus | Prepush Check |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Phase 1** | P0 | VoIP Calling Bugfixes & Device Preflight Modal | **HOÀN THÀNH** | PASS / PASS | HARD GATES PASSED |
| **Phase 2** | P0 | Composer & Attachment Draft Overhaul (E1 Parity) | **HOÀN THÀNH** | PASS / PASS | HARD GATES PASSED |
| **Phase 3** | P0 | Message UX Parity & Local File Lifecycle | **HOÀN THÀNH** | PASS / PASS | HARD GATES PASSED |
| **Phase 4** | P1 | 5 Server Sync & Social Flows | **HOÀN THÀNH** | PASS / PASS | HARD GATES PASSED |
| **Phase 5** | P1 | Native Voice Messages & AI Transcription | **HOÀN THÀNH** | PASS / PASS | HARD GATES PASSED |
| **Phase 6** | P1/P2 | zCloud Rehydration, E2EE Push (551), Retry & Responsive | **HOÀN THÀNH** | PASS / PASS | HARD GATES PASSED |

---

## 1. Mục tiêu Phase 4
Đấu nối trọn vẹn 5 hệ thống giao tiếp server đã có sẵn module protocol vào pipeline khởi động nền và hoàn thiện trải nghiệm UI/UX:
1. **Đánh dấu Chưa đọc Thủ công (Mark as Unread) 2 Chiều Server**: `/api/conv/getUnreadMark`, `/api/conv/addUnreadMark`, `/api/conv/removeUnreadMark`.
2. **Hội thoại Đã lưu trữ (Archived Chats) Đồng bộ**: `/api/archivedchat/list`, `/api/archivedchat/update`.
3. **Chat Ẩn Đồng bộ Mã PIN Server (Hidden Chats)**: `/api/hiddenconvers/get-all`, `/api/hiddenconvers/update-pin`, `/api/hiddenconvers/add-remove`.
4. **Nhãn Hội thoại Đồng bộ Server (Labels)**: `/api/convlabel/get`, `/api/convlabel/update`.
5. **Trạng thái Hoạt động Realtime (`lastOnline`) & Mute Đồng bộ (`getmute` / `setmute`)**: `/api/social/profile/lastOnline`, `/api/social/profile/getmute`, `setmute`.

---

## 2. Chi tiết Hợp đồng Kỹ thuật Phase 4 (Đã đối soát 100% với Codebase)

### Task 4.1: Đánh dấu Chưa đọc Thủ công 2 Chiều Server
- **Tệp liên quan:** `src/unread.rs`, `src/gui/worker.rs`, `src/gui.rs`, `src/gui/workspace.rs`
- **Hợp đồng & Sử dụng Mã hiện có:**
  - `Thread` struct: Sử dụng trường có sẵn `thread.unread.marked_unread: bool` (trong `crate::unread::UnreadState`). **KHÔNG** thêm trường thừa `is_manual_unread`.
  - `WorkerCmd`: Sử dụng `WorkerCmd::ManualMark { owner, generation, target, group, action: crate::unread::MarkAction }` có sẵn.
  - `GuiEvent`: Sử dụng `GuiEvent::ManualMarks { owner, generation, mutation, result: Result<Vec<ManualUnreadMark>, String> }` có sẵn.
  - Handler trong `gui.rs`: Đã có sẵn gọi `crate::unread::merge_manual_unread_marks(&mut self.threads, &marks)`.
- **Nhiệm vụ triển khai:**
  1. `src/gui/worker.rs`:
     - Trong startup background preload (vòng lặp metadata/login): Gọi `crate::unread::fetch_manual_unread_marks(&account).await` cùng lúc với các tác vụ khác qua `tokio::join!` $\rightarrow$ phát `GuiEvent::ManualMarks`.
  2. `src/gui/workspace.rs`:
     - Vẽ badge chưa đọc thủ công: Trên sidebar thread row, khi `unread_n == 0 && t.unread.marked_unread`, vẽ một chấm tròn đỏ nổi bật (`pal.danger`, bán kính 4.5px) ở góc phải nơi hiển thị badge.
     - Khi mở hội thoại (`open_thread`): Nếu `thread.unread.marked_unread == true`, lập tức xóa cờ local `thread.unread.marked_unread = false` và gọi `self.manual_mark_command(tid.clone(), is_group, crate::unread::MarkAction::Remove)` để server Zalo xóa dấu chưa đọc.
     - Kiểm tra menu chuột phải: Đã có sẵn "Đánh dấu chưa đọc trên máy chủ" và "Bỏ dấu chưa đọc trên máy chủ".

---

### Task 4.2: Hội thoại Lưu trữ (Archived Chats) Đồng bộ
- **Tệp liên quan:** `src/archive.rs`, `src/gui/worker.rs`, `src/gui.rs`, `src/gui/workspace.rs`
- **Hợp đồng & Sử dụng Mã hiện có:**
  - `Thread` struct: Sử dụng trường có sẵn `thread.presentation.archived: Option<bool>`. **KHÔNG** thêm trường thừa `is_archived`.
  - `WorkerCmd`: Sử dụng `WorkerCmd::Archive { owner, generation, target, group, archived: Option<bool> }` có sẵn.
  - `GuiEvent`: Sử dụng `GuiEvent::Archive { owner, generation, mutation, result: Result<crate::archive::Snapshot, String> }` có sẵn.
  - Handler trong `gui.rs`: Đã có sẵn cập nhật `thread.presentation.archived` cho mọi thread.
  - Lọc hội thoại lưu trữ: Đã có sẵn `ThreadFilter::Archived` ("📁 Tin nhắn đã lưu trữ") và đã ẩn thread lưu trữ ở danh sách chat chính.
- **Nhiệm vụ triển khai:**
  1. `src/gui/worker.rs`:
     - Trong startup background preload: Thêm `crate::archive::execute(&account, "", false, None).await` vào `tokio::join!` $\rightarrow$ phát `GuiEvent::Archive` để nạp snapshot lưu trữ từ server ngay khi mở app.
  2. `src/gui/workspace.rs`:
     - Thêm tùy chọn vào menu chuột phải của thread sidebar:
       - Nếu `t.presentation.archived == Some(true)`: *"Đưa về hội thoại chính"* (Bỏ lưu trữ).
       - Ngược lại: *"Lưu trữ cuộc trò chuyện"*.
       - Click $\rightarrow$ gọi `self.archive_command(target_id.clone(), target_group, Some(!is_archived))`.

---

### Task 4.3: Chat Ẩn Đồng bộ Mã PIN Server (Hidden Chats)
- **Tệp liên quan:** `src/hidden_chat.rs`, `src/gui/worker.rs`, `src/gui.rs`, `src/gui/workspace.rs`
- **Hợp đồng & Sử dụng Mã hiện có:**
  - `WorkerCmd`: Sử dụng `WorkerCmd::Hidden { owner, generation, action: crate::hidden_chat::Action }` có sẵn.
  - `GuiEvent`: Sử dụng `GuiEvent::Hidden { owner, generation, view: crate::hidden_chat::Snapshot, result: Result<(), String> }` có sẵn.
  - Startup preload: `hidden_chat::execute(&account, &Action::Read)` và khôi phục cache ĐÃ ĐƯỢC CHẠY trong vòng lặp metadata ở `worker.rs`.
  - Bộ lọc: `self.private_access(&t.id, t.is_group)` đã lọc các thread ẩn ra khỏi sidebar và tìm kiếm khi chưa mở khóa.
  - Giao diện modal: `hidden_panel.rs` đã hoàn chỉnh để nhập PIN, khóa, mở khóa, đổi PIN, thêm/xóa chat ẩn.
- **Nhiệm vụ triển khai:**
  - Trong thanh tìm kiếm chat ở sidebar (`workspace.rs`):
    - Khi người dùng nhập chuỗi 4 chữ số vào ô tìm kiếm và **nhấn Enter** (hoặc hiển thị nút bấm gợi ý *"Mở khóa hội thoại ẩn"*):
      - Gửi `WorkerCmd::Hidden { owner, generation, action: Action::Unlock(Zeroizing::new(pin)) }`.
    - **Lưu ý an toàn (BẮT BUỘC):** Tuyệt đối KHÔNG tự động unlock trên sự kiện gõ phím thay đổi chuỗi (`on_change`) để tránh bị khóa an toàn 60 giây khi nhập sai 5 lần (`failures >= 5` $\rightarrow$ `blocked_until = 60s`). Chỉ kích hoạt khi người dùng nhấn Enter có chủ đích.

---

### Task 4.4: Nhãn Hội thoại Đồng bộ Server (Labels)
- **Tệp liên quan:** `src/labels.rs`, `src/gui/labels_panel.rs`, `src/gui/worker.rs`, `src/gui.rs`
- **Hợp đồng & Sử dụng Mã hiện có:**
  - `WorkerCmd`: Sử dụng `WorkerCmd::Labels { owner, generation, action: crate::labels::Action }` có sẵn.
  - `GuiEvent`: Sử dụng `GuiEvent::Labels` có sẵn (gán `self.labels_panel.snapshot = Some(snapshot)`).
  - Panel nhãn: `labels_panel.rs` đã hoàn thiện giao diện xem, tạo, đổi tên, xóa, gán nhãn cho thread.
- **Nhiệm vụ triển khai:**
  1. `src/gui/worker.rs`:
     - Trong startup background preload: Thêm `crate::labels::execute(&account, &crate::labels::Action::Read).await` vào `tokio::join!` $\rightarrow$ phát `GuiEvent::Labels` để nạp danh sách nhãn từ server ngay khi khởi động.
  2. `src/gui/workspace.rs`:
     - Đảm bảo thanh phân loại hiển thị đúng và bộ lọc theo nhãn hoạt động mượt mà với snapshot nhãn từ server.

---

### Task 4.5: Trạng thái Online Realtime & Mute Đồng bộ
- **Tệp liên quan:** `src/last_activity.rs`, `src/mute_sync.rs`, `src/gui/worker.rs`, `src/gui.rs`, `src/gui/workspace.rs`
- **Hợp đồng & Sử dụng Mã hiện có:**
  - **Mute Sync:**
    - `sync_mute_snapshot` trong `worker.rs` ĐÃ ĐƯỢC CHẠY khi khởi động: nạp từ server qua `fetch_mute_snapshot`, reconcile với SQLite, và phát `GuiEvent::ConversationPreferences`.
    - `WorkerCmd::SetConversationPreference` ĐÃ ĐƯỢC NỐI gọi `crate::mute_sync::set_remote_mute`.
    - `live_updates.rs` ĐÃ CHẶN thông báo và âm thanh khi thread bị mute (`!thread.unread.is_muted() && !muted.contains(&thread.id)`).
  - **Last Activity:**
    - Module `src/last_activity.rs` đã có hàm `read(&s, &target, official)`.
    - `GuiEvent::ActivityUpdated { owner, uid, status: crate::last_activity::Status }` đã có sẵn trong `gui.rs`.
- **Nhiệm vụ triển khai:**
  1. **Mute UI:**
     - Trên sidebar thread row (`workspace.rs`): Khi `is_muted == true` (lấy từ `conversation_preferences`), vẽ icon chuông gạch chéo `🔕` (FontId 10.5) cạnh timestamp hoặc tên thread.
     - Cập nhật menu chuột phải: Đổi text hiển thị từ `"Tắt thông báo (trên máy này)"` thành `"Tắt thông báo"` (vì đã đồng bộ 2 chiều với server Zalo).
  2. **Last Online / Trạng thái Hoạt động:**
     - `WorkerCmd`: Thêm `FetchLastOnline { owner: String, target: String, official: bool }`.
     - `worker.rs`: Khi nhận `WorkerCmd::FetchLastOnline`, gọi `crate::last_activity::read(&account, &target, official).await`. Nếu thành công, phát `GuiEvent::ActivityUpdated { owner, uid: target, status }`.
     - `ZaloGuiApp` (`src/gui.rs`): Thêm trường `pub presence_map: std::collections::HashMap<String, (crate::last_activity::Status, std::time::Instant)>`. Trong handler `GuiEvent::ActivityUpdated`, luôn cập nhật vào `presence_map` (không bị giới hạn bởi `contact_panel.target`).
     - Khi mở hội thoại 1-1 (`open_thread` trong `workspace.rs`): Nếu là chat 1-1 (`!is_group && !is_cloud`), kiểm tra `presence_map`. Nếu chưa có hoặc đã quá 60 giây, gửi `WorkerCmd::FetchLastOnline`.
     - Trong Central Chat Header (`workspace.rs`):
       - Avatar 1-1: Vẽ cutout online dot màu xanh lá (`pal.accent_online`, bán kính 4.5px, viền nền) ở góc dưới-phải avatar khi trạng thái là `LastActivity(ts)` với `now - ts <= 180_000` (hoạt động trong vòng 3 phút).
       - Dòng text phụ đề dưới tên đối phương:
         - Nếu `LastActivity(ts)`:
           - `<= 3 phút`: Hiển thị `"Đang hoạt động"` với màu xanh lá `pal.accent_online`.
           - `< 60 phút`: Hiển thị format `"Truy cập {mins} phút trước"`.
           - `< 24 giờ`: Hiển thị format `"Truy cập {hours} giờ trước"`.
           - `>= 24 giờ`: Hiển thị `"Truy cập hôm qua"` hoặc ngày cụ thể.
         - Nếu `PrivacyUnavailable` hoặc `Unavailable`: Fallback về trạng thái kết bạn mặc định (`"Đã kết bạn"`).

---

## 3. Kiến trúc Preload Khởi động Đồng thời (Concurrent Startup Preload)
Để đảm bảo thời gian khởi động app luôn dưới 1 giây và không bao giờ block UI thread:
- Trong `src/gui/worker.rs`, các lệnh nạp metadata từ server được gộp chạy đồng thời bằng `tokio::join!`:
```rust
let unread_fut = crate::unread::fetch_manual_unread_marks(&account);
let archive_fut = crate::archive::execute(&account, "", false, None);
let labels_fut = crate::labels::execute(&account, &crate::labels::Action::Read);
let (unread_res, archive_res, labels_res) = tokio::join!(unread_fut, archive_fut, labels_fut);
```
- Kết quả được phát lần lượt về `gui_tx` (`GuiEvent::ManualMarks`, `GuiEvent::Archive`, `GuiEvent::Labels`).
- Tất cả I/O SQLite đều chạy trong `tokio::task::spawn_blocking`.
- Giao diện người dùng render ngay lập tức từ dữ liệu SQLite đã cache, sau đó cập nhật phản chiếu (reactive render) ngay khi các sự kiện nền trả về.

---

## 4. Invariants & Tiêu chí Nghiệm thu (Non-Negotiables)
- Tuyệt đối không gọi I/O hoặc network trên UI thread.
- Duy trì nguyên tắc Độc nhất Nguồn Chân lý (Single Source of Truth), không tạo trường dữ liệu trùng lặp (`marked_unread` và `archived` đã có vị trí chuẩn trên `Thread`).
- Không tạo UI stubs: mọi nút bấm (lưu trữ, tắt thông báo, đánh dấu chưa đọc, mở khóa PIN) đều phải đi trọn vẹn luồng `UI -> WorkerCmd -> Server API -> GuiEvent -> App State -> Reactive UI`.
- Chạy `cargo check --locked` và `powershell scripts/audit-prepush.ps1` đạt 100% trước khi kết thúc Phase 4.
