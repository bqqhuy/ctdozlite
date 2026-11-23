# Kế hoạch khắc phục Audit Vòng 4 (Codex Round 4 Remediation Plan)

Ngày: 2026-11-23
Trạng thái hiện tại: Codex đã sửa bốn phát hiện audit vòng 5 theo yêu cầu người dùng; đã qua kiểm thử local (1115 library, 4 integration, Clippy/format đạt). Các mục AMBIGUOUS/native vẫn mở. Các phần kế hoạch bên dưới ghi lịch sử triển khai, không phải phê duyệt nghiệm thu.
Mục tiêu: Khắc phục triệt để 3 phát hiện còn lại của Codex Audit Vòng 4, bảo toàn toàn bộ 7 sửa đổi của Vòng 3 và các vòng trước.

---

## 1. Trạng thái Baseline & Working Tree

- **Baseline Commit được Audit:** `f33f43a4c0b7e27d15d36393ad0556728b3d547f`
- **Git Branch:** `main`
- **Working Tree:** 14 files đã chỉnh sửa, bảo toàn 100% thay đổi không commit/push.
- **Quy tắc bảo tồn:**
  1. Payload password login đúng nhánh bật/tắt mã hóa, kể cả fallback 19000 (không salt, raw pwd).
  2. Clone runtime cũ không đổi policy runtime mới (cấp mới `Arc<AtomicBool>`, active guard).
  3. Metadata rỗng không được coi là config sync chính thức (`sync_2_pc_authoritative`).
  4. Event ID=0 không đè operation đang chạy.
  5. Fatal không thành Ready hoặc được retry sai.
  6. Backup chỉ giữ write permit theo batch; test concurrency không tự resume khi timeout.
  7. Auto-ZIP, recall/delete, realtime và checkpoint không bị đổi ngoài phạm vi.

---

## 2. Chi tiết 3 phát hiện Vòng 4 và Phương án xử lý

### Phát hiện 1: Chống phát lặp auto-sync trong khi chờ ACK (`src/gui.rs:7534`)

1. **Nguyên nhân và đường chạy thực:**
   - Trong `src/gui.rs:7534` (`try_trigger_pending_auto_sync`), `intent` chỉ là struct tĩnh. Khi thỏa mãn điều kiện sẵn sàng, code gửi `WorkerCmd::SyncPhoneRecentFenced` nhưng không chuyển trạng thái intent sang "đang chờ worker chấp nhận".
   - Nếu `try_trigger_pending_auto_sync` được gọi 2 lần trước khi worker xử lý (ví dụ qua 2 frame liên tiếp hoặc do cả 2 event WS ready + DB ready cùng đến), nó sẽ gửi 2 lệnh `WorkerCmd::SyncPhoneRecentFenced` giống hệt nhau xuống channel worker!
2. **State / Event / Command thay đổi:**
   - Thêm `enum AutoSyncIntentStage`:
     ```rust
     #[derive(Clone, Debug, PartialEq, Eq)]
     pub(crate) enum AutoSyncIntentStage {
         Pending,
         AwaitingAcceptance { dispatched_at: std::time::Instant },
     }
     ```
   - Cập nhật `struct PendingAutoSyncIntent`:
     ```rust
     #[derive(Clone, Debug, PartialEq, Eq)]
     pub(crate) struct PendingAutoSyncIntent {
         pub intent_id: u64,
         pub owner: String,
         pub runtime_id: usize,
         pub source: crate::sync_coordinator::SyncSource,
         pub stage: AutoSyncIntentStage,
     }
     ```
   - Thêm `intent_id: Option<u64>` và `origin: SyncOrigin` vào `WorkerCmd::SyncPhoneRecentFenced`.
   - Thêm 2 GuiEvent mới để worker xác nhận:
     - `GuiEvent::AutoSyncIntentAccepted { owner, runtime_id, intent_id, operation_id }`
     - `GuiEvent::AutoSyncIntentRejected { owner, runtime_id, intent_id, temporary: bool, reason: String }`
   - Khi `try_trigger_pending_auto_sync` gửi lệnh:
     - Chuyển ngay `intent.stage = AutoSyncIntentStage::AwaitingAcceptance { dispatched_at: Instant::now() }`.
     - Nếu channel đóng (`WORKER_TX.send` lỗi): dọn dẹp ngay `self.pending_auto_sync = None;` tránh bị kẹt.
     - Nếu `intent.stage != AutoSyncIntentStage::Pending`: bỏ qua không gửi thêm command.
   - Khi nhận `AutoSyncIntentAccepted`: nếu khớp `intent_id`, xóa sạch `self.pending_auto_sync = None;`.
   - Khi nhận `AutoSyncIntentRejected`:
     - Nếu `temporary == true`: chuyển `intent.stage = AutoSyncIntentStage::Pending` để chỉ thử lại khi có sự kiện readiness mới, không lặp vô hạn.
     - Nếu `temporary == false`: xóa `self.pending_auto_sync = None;`.
   - Logout / account switch: xóa sạch `self.pending_auto_sync = None;`.
3. **Invariant bảo toàn:**
   - Không tự ý retry operation đã chấp nhận.
   - Response/intent cũ từ owner/runtime cũ bị drop.
   - Giữ nguyên guard chống sync đồng thời ở worker (`phone_sync_running.swap(true, ...)`).
4. **Test bắt đúng triệu chứng:**
   - Test harness gọi `try_trigger_pending_auto_sync` 2 lần trước khi có ACK: assert kênh nhận chính xác đúng 1 `WorkerCmd`.
   - Config/readiness lặp trong lúc `AwaitingAcceptance`: vẫn chỉ có 1 command.
   - ACK đúng `intent_id`: chuyển state sạch sẽ.
   - ACK/rejection từ intent cũ / runtime cũ: bị drop.
   - Temporary rejection: quay lại Pending, thử lại 1 lần duy nhất khi có readiness mới.
   - Channel đóng: intent không bị kẹt.
5. **Bằng chứng:**
   - E2: Zalo PC `SyncService.attemptAutoSync`: dispatch transition sang awaiting state và correlate intent ID.

---

### Phát hiện 2: Tách nguồn kích hoạt khỏi loại sync và áp đúng gate (`src/server_config.rs:236`)

1. **Nguyên nhân và đường chạy thực:**
   - Hiện tại `is_feature_enabled_by_sync_source` trong `server_config.rs` dùng chung `enable_on_sync_source_setting` cho cả `Sync2Full`, `Sync2Recent`, `Sync2Rescan`.
   - Auto-sync phát sinh từ NewLogIn gửi lệnh Recent nên bị gán nhầm vào gate `enable_on_sync_source_setting` của Settings!
2. **State / Event / Command thay đổi:**
   - Phân định rõ 2 khái niệm:
     - **SyncOrigin (Nguồn kích hoạt):** `Setting` (nút bấm từ cài đặt), `AutoSync` (tự động khi đăng nhập mới), `Banner` (bấm gợi ý banner), `Legacy`.
     - **SyncMode (Phạm vi/loại đồng bộ):** `Full`, `Recent`, `Rescan`.
   - Trong `Sync2PcConfig`:
     - Bổ sung struct `AutoSyncConfig { pub enable: bool }` (default `true`).
     - Thêm trường `pub auto_sync: AutoSyncConfig` vào `Sync2PcConfig`.
     - Trong `apply_sync_2_pc`: Parse `auto_sync: { enable: bool }` từ payload server config theo đúng schema PC.
   - Thêm phương thức `is_feature_enabled_by_origin(&self, origin: SyncOrigin) -> bool`:
     - Global `!self.enable` $\rightarrow$ return `false`.
     - `SyncOrigin::Setting` $\rightarrow$ return `self.enable_on_sync_source_setting`.
     - `SyncOrigin::AutoSync` $\rightarrow$ return `self.auto_sync.enable`.
     - `SyncOrigin::Banner` $\rightarrow$ return `self.enable_on_sync_source_setting`.
   - Truyền `origin: SyncOrigin` từ UI/intent qua `WorkerCmd::SyncPhoneRecentFenced` xuống worker và `evaluate_sync_availability`.
3. **Invariant bảo toàn:**
   - Không đổi frame ID, params, hay request protocol wire.
   - Setting tắt nhưng auto bật: auto vẫn chạy được.
   - Setting bật nhưng auto tắt: auto bị chặn, không đi nhờ gate của Setting.
   - Cùng mode Recent: khác origin được đánh giá độc lập.
4. **Test bắt đúng triệu chứng:**
   - Schema test: parse `auto_sync.enable` lồng nhau.
   - Origin test: Setting false, auto true $\rightarrow$ Setting bị chặn, auto được phép.
   - Origin test: Setting true, auto false $\rightarrow$ Setting được phép, auto bị chặn.
   - Global enable false: chặn tất cả các nguồn.
   - Producer/caller test: origin được truyền nguyên vẹn từ UI xuống worker.
5. **Bằng chứng:**
   - E2: `main-startup.41abf8e6af2a55551e3b.js` marker `isFeatureEnabledBySyncSource(e)` khoảng code-point 2519126: SETTING dùng `enable_on_sync_source_setting`, AUTO_SYNC dùng `auto_sync.enable`.

---

### Phát hiện 3: Nối pha optional preload thực tế (`src/gui.rs:4117`, `src/gui/worker.rs:1860`)

1. **Nguyên nhân và đường chạy thực:**
   - `prerequisites_settled` trước đây chỉ được set `true` khi nhận chính `StartupPreloadSettled`!
   - Điều này khiến trạng thái "prerequisites bắt buộc đã xong, optional preload đang chạy" không bao giờ xảy ra trong mã nguồn thực tế (chỉ có trong unit test gán thủ công).
2. **State / Event / Command thay đổi:**
   - Bổ sung `GuiEvent::StartupPrerequisitesSettled { owner, runtime_id }`.
   - Trong worker `run_startup_preload`:
     - Giai đoạn 1 (Bắt buộc): Mở SQLite DB, chạy migrations, kiểm tra SEK và validate session (`s.validate_session()`).
     - Khi session hợp lệ và DB sẵn sàng: worker gửi `GuiEvent::StartupPrerequisitesSettled { owner, runtime_id }`!
     - Giai đoạn 2 (Tùy chọn): Worker tiến hành preload threads, hydrate E2EE peers, chuẩn bị cache. Khi hoàn thành: worker gửi `GuiEvent::StartupPreloadSettled`.
   - Trong `src/gui.rs`:
     - Nhận `StartupPrerequisitesSettled`:
       - Đặt `prerequisites_settled = true`.
       - Đặt `started_at = Instant::now()` (bắt đầu tính timeout cho phần optional preload từ đúng mốc này, không tính gộp thời gian login).
       - Nếu `!keep_loading_during_preload.enable`: chuyển ngay `StartupState::Ready`.
       - Ngược lại: giữ màn hình loading chờ optional preload hoặc timeout.
     - Nhận `StartupPreloadSettled`:
       - Nếu `keep_loading_during_preload.enable == true`: kích hoạt `settle_deadline = Some(Instant::now() + delay)` tuân thủ `screen_dismiss_delay_ms`.
     - Trong `advance_startup_state`:
       - `!prerequisites_settled`: tuyệt đối không chuyển `Ready` (dù quá timeout).
       - `prerequisites_settled && keep_loading_during_preload.enable`:
         - Nếu `now.saturating_duration_since(*started_at) >= *timeout`: giải phóng loading màn hình và chuyển `Ready`.
         - Nếu có `settle_deadline` và `now >= deadline`: chuyển `Ready`.
3. **Invariant bảo toàn:**
   - DB/auth prerequisite pending không bị timeout bỏ qua.
   - `FatalError` không bao giờ biến thành `Ready`.
   - Không sleep cố định trên UI thread.
   - Event đến trễ / runtime cũ không làm kẹt màn hình.
4. **Test bắt đúng triệu chứng:**
   - Pipeline test từ worker: phát `StartupPrerequisitesSettled`, giữ barrier ở optional preload $\rightarrow$ UI thực sự bước vào `prerequisites_settled: true`, `StartupState::Loading`.
   - Thay đổi 2 mốc timeout khác nhau $\rightarrow$ thời điểm giải phóng màn hình khác nhau đúng bằng chênh lệch timeout.
   - Preload settled sớm $\rightarrow$ áp dụng đúng dismiss delay.
   - Mandatory lỗi $\rightarrow$ `StartupFatalError`, không bao giờ vào optional hay Ready.
5. **Bằng chứng:**
   - E2: Zalo PC `keep_loading_during_preload` (`main-startup.js:920`).

---

## 3. Thứ tự triển khai

1. **Bước 1 (Phát hiện 2):** Cập nhật `src/sync_coordinator.rs` (SyncOrigin), `src/server_config.rs` (AutoSyncConfig, is_feature_enabled_by_origin) và unit test.
2. **Bước 2 (Phát hiện 1):** Cập nhật `src/gui.rs` (PendingAutoSyncIntent stage, intent_id, dispatch/ACK routing), `src/gui/worker.rs` (AutoSyncIntentAccepted/Rejected) và test lặp command.
3. **Bước 3 (Phát hiện 3):** Tách `StartupPrerequisitesSettled` và `StartupPreloadSettled` trong `src/gui/worker.rs` và `src/gui.rs`, áp timeline chuẩn.
4. **Bước 4 (Review & Verification):** Duyệt qua Reviewer A & Reviewer B, chạy `cargo test --lib`, `cargo test --test desktop_backup_v2`, `audit-prepush.ps1`.


## Codex: sửa bốn phát hiện audit vòng 5 (23/11/2026)

Phạm vi hiện tại là bốn lỗi đã được người dùng giao sửa trực tiếp, trên working tree hiện hữu; không reset/commit/push. Bằng chứng: [re-notes/73](../../re-notes/73-sync-origin-timeouts-and-startup-completion.md).

| Việc | Đường chạy cuối | Tiêu chí kiểm chứng |
| --- | --- | --- |
| Timeout AUTO độc lập | Worker truyền origin vào transfer; entry point chọn snapshot timeout; confirmation/wake/message rounds nhận snapshot | Fixture manual 10s / AUTO 45s, đủ ba timeout, config cập nhật không đổi snapshot đang dùng |
| Gate Banner | Banner dùng global Sync2, không dùng gate Settings; clear-error của banner cũng tính availability theo Banner | Settings tắt nhưng Banner được phép, vẫn chặn nếu DB/socket/quyền chưa sẵn sàng |
| V1 chủ động | Legacy không dùng gate riêng Sync2; không tự fallback | Sync2 tắt không vô hiệu V1; quyền/DB/socket vẫn bắt buộc |
| Optional preload thật | Sau mandatory event, worker await hydrate local rồi mới phát settled; lỗi optional cho settle, runtime đóng bỏ event | Barrier ở producer thật nối đến GUI, timeout 800/2000ms, late completion, optional lỗi, runtime đóng và payload peer sai runtime |

Bảo toàn ACK/intent chống lặp, ID=0 fencing, password payload, runtime cipher isolation, metadata authority, auto-ZIP và writer gate. Không đổi request/frame/crypto. Đã bỏ hydrate trùng sau QR/password; auth startup lấy snapshot session sau apply config.

Kết quả kiểm chứng cuối được ghi tại audit dossier. ChangeDevice/QueueEvict, ZIP-08 và native phone sync không tự chuyển thành hoàn tất.


## Codex — vòng sửa/audit liên tiếp ngày 23/11/2026

Yêu cầu: sửa đến khi hai lượt audit liên tiếp không phát hiện vấn đề trong phạm vi sync/config đang sửa. Hai lượt do cùng Codex thực hiện theo hai phương pháp, không phải hai reviewer độc lập. Không commit/push/tag/release.

Các phát hiện đã xử lý trước khi khóa candidate:
- Cache hội thoại phải đi cùng mandatory-ready, được UI áp dụng trước Ready; optional peer hydrate không giữ cache.
- Cache đến trễ chỉ bổ sung lịch sử/metadata thiếu, không ghi đè alias/avatar/unread/plaintext/reaction/pin/star mới; recall và xóa cục bộ vẫn đơn điệu.
- Worker đóng sau dispatch nhưng trước ACK: hủy intent chưa rõ kết quả, không tự resend; quan sát channel đóng bằng heartbeat UI.
- Node sync null/array/chuỗi lỗi không được tạo authority hay thay policy đã biết.
- Stress song song phát hiện test app bootstrap=false vẫn spawn worker thật khi logout/profile switch. Giữ cờ bootstrap qua cả hai transition; thêm regression xác nhận không phát RequestQr/RefreshThreads. Không đổi đường production bootstrap=true.

Quy tắc đếm: phát hiện bất kỳ vấn đề nào phải sửa rồi đặt lại số lượt sạch về 0. Lượt stress ban đầu thất bại ở vòng 2 (13 đạt, 1 lỗi), nên các kết luận sạch trước đó không được cộng tiếp. Candidate cuối được hash cả src/tests và các file build; giữ nguyên giữa hai lượt. Kết quả thực chạy và giới hạn ghi tại audit dossier.

Stress tiếp theo: 66 vòng đầu đạt, vòng 67 thất bại (14 đạt, 1 lỗi). Đây là nhiễu hộp lệnh toàn cục còn lại giữa nhiều app test; tiếp tục đặt bộ đếm về 0. Mock mailbox được tách thread-local dưới cfg(test), kèm regression cross-thread. Mailbox Mutex production không đổi. Bản ứng viên trước đó có 1120 test đạt nhưng không được kết luận đã hội tụ.


### Kết quả vòng lặp cuối

Candidate `c416614389549a0d30c8beb533717b2fa11441886bd636837bb8e5265c32c7ba` đã đạt hai lượt Codex audit liên tiếp không phát hiện thêm vấn đề trong phạm vi sync/config. Lượt 1: rà luồng dữ liệu + 1121 library test đạt/11 ignored + 100 vòng stress 8 thread. Lượt 2: rà nhánh lỗi/thứ tự sự kiện + 58 test nhắm mục tiêu + 100 vòng stress 16 thread + 4 integration backup đạt + Clippy all-targets `-D warnings` exit 0. Nguồn không đổi giữa hai lượt, HEAD vẫn f33f43a. Hồ sơ chi tiết và giới hạn: [audit dossier](2026-11-23-sync-config-audit-dossier.md). Native acceptance, ZIP-08 và ChangeDevice/QueueEvict vẫn mở; không commit/push/release.
