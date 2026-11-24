# .agent/TASKS.md - Trạng thái Kế hoạch Toàn diện Zalo Parity

## Phase 1 (P0): Triệt tiêu lỗi chuông VoIP dai dẳng & Device Preflight
- [x] Task 1.0: Trình ADVISOR phê duyệt Phase 1 (ĐÃ PHÊ DUYỆT: PLAN_APPROVED)
- [x] Task 1.1: Sửa khớp danh tính `actor_is_self` (`src/call.rs`, `src/call_lifecycle.rs`)
- [x] Task 1.2: Xử lý `ReceiveCall` (nghe nơi khác), `MissCall`, dừng stream CPAL & đóng modal (`src/gui/worker.rs`, `src/gui.rs`, `src/media.rs`)
- [x] Task 1.3: Watchdog timer 65s ngắt chuông an toàn & Window close handler (`src/gui/modals.rs`, `src/gui.rs`)
- [x] Task 1.4: Preflight Dialog kiểm tra Mic/Loa trước khi gọi (`src/gui.rs`, `src/gui/modals.rs`, `src/gui/workspace.rs`)
- [x] Task 1.5: Review độc lập Reviewer A & Reviewer B cho Phase 1 (ĐỒNG THUẬN: PASS / PASS)
- [x] Task 1.6: Prepush check (`powershell scripts/audit-prepush.ps1` - HARD GATES PASSED)

## Phase 2 (P0): Tái thiết kế Toàn diện Composer & Đính kèm Tệp (E1 Parity)
- [x] Task 2.0: Trình ADVISOR phê duyệt chi tiết Phase 2 (ĐÃ PHÊ DUYỆT: PLAN_APPROVED)
- [x] Task 2.1: Toolbar 9 nút chuẩn & popup menu con (...) (`src/gui/workspace.rs`)
- [x] Task 2.2: Gỡ bỏ chặn gửi file với caption text (`src/gui/file_drop.rs:548-570`)
- [x] Task 2.3: Tái cấu trúc khu vực thẻ tệp chờ: header Xóa tất cả, thẻ vuông 72x72px bo góc 8px kèm nút X, nút + nét đứt (`src/gui/file_drop.rs`)
- [x] Task 2.4: Chuyển đổi Nút Thích -> Gửi xanh Zalo khi có file/text; sticker gợi ý nổi; @ mention menu (`src/gui/workspace.rs`)
- [x] Task 2.5: Modal chọn gửi danh thiếp chuẩn Zalo với search + filter chips (`src/gui/workspace.rs`, `src/gui.rs`)
- [x] Task 2.6: Review độc lập Reviewer A & Reviewer B cho Phase 2 (ĐỒNG THUẬN: PASS / PASS)
- [x] Task 2.7: Prepush check (`powershell scripts/audit-prepush.ps1` - HARD GATES PASSED)

## Phase 3 (P0): Tương tác Tin nhắn Chuẩn UX Zalo & Vòng đời Tệp Local
- [x] Task 3.0: Trình ADVISOR phê duyệt chi tiết Phase 3 (ĐÃ PHÊ DUYỆT: PLAN_APPROVED)
- [x] Task 3.1: Xóa bỏ trễ hover 150ms (`src/gui/messages.rs`)
- [x] Task 3.2: Thanh công cụ MQA nổi `80×24px` (`src/gui/messages.rs`, `src/gui/theme.rs`)
- [x] Task 3.3: Menu Reaction Capsule `236×38px` với 6 emoji zoom 1.25x (`src/gui/messages.rs`)
- [x] Task 3.4: Reaction Pill Badge đè 12px mép đáy bubble tin nhắn (`src/gui/messages.rs`)
- [x] Task 3.5: Mở tệp trực tiếp (tệp gửi mở `file_path` ổ cứng, tệp nhận tự lưu cache `%TEMP%` và mở) (`src/gui/messages.rs`, `src/gui/workspace.rs`)
- [x] Task 3.6: Review độc lập Reviewer A & Reviewer B cho Phase 3 (ĐỒNG THUẬN: PASS / PASS)
- [x] Task 3.7: Prepush check (`powershell scripts/audit-prepush.ps1` - HARD GATES PASSED)

## Phase 4 (P1): Đấu nối 5 Luồng Đồng bộ Server & Xã hội Bị Bỏ Quên
- [x] Task 4.0: Trình ADVISOR phê duyệt chi tiết Phase 4 (ĐÃ PHÊ DUYỆT: PLAN_APPROVED)
- [x] Task 4.1: Đánh dấu chưa đọc thủ công 2 chiều (`getUnreadMark`, `addUnreadMark`, `removeUnreadMark`) (`src/gui/worker.rs`, `src/gui/workspace.rs`)
- [x] Task 4.2: Chat ẩn đồng bộ mã PIN Server (`hiddenconvers/get-all`, `update-pin`) (`src/gui/worker.rs`, `src/gui/workspace.rs`)
- [x] Task 4.3: Đồng bộ danh sách Hội thoại Đã lưu trữ (`archivedchat/list`, `archivedchat/update`) (`src/gui/worker.rs`, `src/gui/workspace.rs`)
- [x] Task 4.4: Đồng bộ Nhãn hội thoại (`convlabel/get`, `convlabel/update`) (`src/gui/worker.rs`, `src/gui/workspace.rs`)
- [x] Task 4.5: Trạng thái Online / Last Seen (`lastOnline`) & Mute hội thoại (`getmute`) (`src/gui/worker.rs`, `src/gui/workspace.rs`, `src/gui.rs`)
- [x] Task 4.6: Review độc lập Reviewer A & Reviewer B cho Phase 4 (ĐỒNG THUẬN: PASS / PASS)
- [x] Task 4.7: Prepush check (`powershell scripts/audit-prepush.ps1` - HARD GATES PASSED)

## Phase 5 (P1): Tin nhắn Thoại Native & Dịch thuật Giọng nói AI
- [x] Task 5.0: Trình ADVISOR phê duyệt chi tiết Phase 5 (ĐÃ PHÊ DUYỆT: PLAN_APPROVED)
- [x] Task 5.1: Thanh ghi âm Composer bám đáy (đếm giây, sóng VU meter qua `cpal`)
- [x] Task 5.2: Gửi tin thoại chuẩn giao thức (DM CMD 11148, Group CMD 11169)
- [x] Task 5.3: Waveform Bubble Player & Nút AI dịch giọng nói (CMD 13394)
- [x] Task 5.4: Review độc lập Reviewer A & Reviewer B cho Phase 5 (ĐỒNG THUẬN: PASS / PASS)
- [x] Task 5.5: Prepush check (`powershell scripts/audit-prepush.ps1` - HARD GATES PASSED)

## Phase 6 (P1/P2): zCloud Rehydration, E2EE Realtime Push (CMD 551), Retry Động & Bố cục Responsive
- [x] Task 6.0: Trình ADVISOR phê duyệt chi tiết Phase 6 (ĐÃ PHÊ DUYỆT: PLAN_APPROVED)
- [x] Task 6.1: Kích hoạt giải mã realtime WebSocket Push CMD 551 (`CMD_SIGNAL_RECEIVE_1_1`)
- [x] Task 6.2: Phục hồi zCloud Media theo yêu cầu (`verifyCloudMedia`, `getDownloadUrlByNoiseIds`)
- [x] Task 6.3: Cấu hình Retry động `[10, 35, 40, 45, 50, 60]`, Gateway Fallback (`chat-wpa.zalo.me`), `alias/list/v2`
- [x] Task 6.4: Bố cục Responsive & Narrow Mode cho màn hình nhỏ (< 720px thu gọn sidebar về 72px)
- [x] Task 6.5: Review độc lập Reviewer A & Reviewer B cho Phase 6 (ĐỒNG THUẬN: PASS / PASS)
- [x] Task 6.6: Prepush check (`powershell scripts/audit-prepush.ps1` - HARD GATES PASSED)

## Phase 7 (Audit Remediation): Sửa Ba Hồi quy Audit 24/11/2026
- [x] Task 7.1: Metadata Snapshot Overwrite (Monotonic Revisions `revision: u64`, `mutation: bool`, loại bỏ `generation = 0` wildcard, gating `AccountMetadata` & cache restore)
- [x] Task 7.2: Preflight / Mic Lifecycle across Account Boundaries (`owner`, `runtime_id` trên `PreflightCallState`, `WorkerCmd::StartCall`, dọn dẹp khi logout/expiry/switch, mock seam CPAL)
- [x] Task 7.3: Presence Cache & Privacy Gating (Session fencing `owner`, `runtime_id` trên `ActivityUpdated`, cổng cứng `show_online == Some(true)` trên Header / Online Dot)
- [x] Task 7.4: Kiểm chứng toàn diện (Full lib test suite 1144 passed, integration `desktop_backup_v2` 4 passed, stress 10 rounds x 8 threads 16/16 passed, clippy 0 warning, fmt & git diff check sạch)
