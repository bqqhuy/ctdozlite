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


## Handoff Antigravity — sửa ba hồi quy audit 24/11/2026

Mục này là kế hoạch khắc phục mới, không phải kết quả đã kiểm thử. Hai lượt audit sạch trước chỉ áp dụng candidate cũ; không áp dụng tự động cho các thay đổi GUI/worker mới. Baseline lúc audit: HEAD c9a274f, working tree có sửa của Antigravity. Không reset/revert công việc khác.

### Prompt giao việc

Bạn là Antigravity, làm việc trong C:/Projects/ZaloLite. Sửa đúng ba hồi quy Codex vừa audit: (1) snapshot metadata nền ghi đè mutation mới; (2) preflight/microphone sống qua ranh giới tài khoản; (3) cache presence vượt owner/runtime và privacy gate. Không mở rộng sang rewrite sync, call protocol hay GUI overhaul.

**Bước 1 — lập kế hoạch trước khi sửa Rust.** Đọc AGENTS.md, các standard API/error, concurrency, testing, security; docs/ZALO_REFERENCE_ANALYSIS.md; re-notes/73 và /74; kế hoạch/dossier audit hiện có. Đọc đúng struct/enum và tất cả call site trước khi thêm field. Cập nhật phần handoff này với: sơ đồ producer -> WorkerCmd -> worker -> GuiEvent -> state -> render; bảng invariant/lỗ hổng/giải pháp/test; danh sách file dự kiến; tradeoff và đường lỗi. Tự rà kế hoạch để tránh sửa một consumer nhưng bỏ đường vòng, rồi triển khai trong phạm vi được giao, không dừng xin xác nhận lại cho lựa chọn thường lệ.

**Bước 2 — sửa snapshot metadata nền.**
- Điểm neo hiện tại: src/gui/worker.rs metadata refresh loop (GET unread/archive/labels, join rồi phát generation=0); src/gui.rs handlers Labels/ManualMarks/Archive và AccountMetadata; src/gui/labels_panel.rs; các helper phát mutation/read tương ứng.
- Không giữ điều kiện generation=0 như wildcard. Cũng không chỉ xóa wildcard rồi vô hiệu hydrate ban đầu hoặc refresh nền.
- Thiết kế một quy tắc revision nhất quán cho từng domain. Gắn owner/runtime và dấu phiên bản khi bắt đầu read, trước HTTP await. So sánh với revision/mutation hiện tại; không cấp revision mới tại lúc response về để làm dữ liệu cũ trông mới hơn. Tận dụng cơ chế đang có nếu phù hợp; giải thích rõ nếu tái dùng search::generation, không suy đoán thứ tự của token.
- Tách ngữ nghĩa snapshot nền và ACK/result của thao tác UI. Snapshot cũ, kể cả error, không được thay dữ liệu, xóa pending, đổi uncertain, phát toast hoàn tất hoặc persist DB của thao tác mới hơn. Response của read UI cũng phải khớp request đang chờ.
- Rà cả AccountMetadata/from_cache: guard !pending riêng lẻ không đủ khi snapshot cũ tới sau ACK. Đảm bảo đường này không vượt revision gate của labels/archive. Không sửa bằng cách chặn vĩnh viễn background refresh sau mutation đầu tiên.
- Hydrate ban đầu hợp lệ và refresh mới sau mutation vẫn hoạt động. Tài khoản khác, runtime cũ của cùng UID, read cũ hoặc response đảo thứ tự đều không được áp dụng.

**Bước 3 — sửa lifecycle preflight.**
- Điểm neo: PreflightCallState/open_preflight_call trong src/gui.rs; clear_presence_state và các nhánh SessionExpired/LoggedOut/ProfileSwitched/login reset; render_preflight_dialog trong src/gui/modals.rs; PreflightMicMonitor trong src/media.rs.
- Gắn ý định preflight với owner/runtime. Hủy/drop preflight và phát tín hiệu dừng monitor ở các ranh giới tài khoản/phiên; dialog cũ không xuất hiện sau đăng nhập mới. Kiểm tra cả re-login cùng UID nhưng runtime mới và thay đổi account trong lúc khởi tạo monitor.
- Kiểm tra scope trước khi xác nhận gọi, và tại worker trước khi thực thi command để đóng race giữa enqueue và account switch. Nếu thêm field/variant WorkerCmd, cập nhật đủ constructor/caller, không thêm giá trị mặc định để lách kiểm tra. Tái dùng đường call hiện hữu; không đổi endpoint, frame ID hay payload server.
- Dừng tài nguyên không được join/sleep/chờ thiết bị trên UI thread. Test dùng monitor/factory giả hoặc test seam tối thiểu; không mở microphone/camera hay gọi Zalo thật. Rà test preflight hiện tại vì nó đang gọi start monitor thật.

**Bước 4 — sửa cache presence và privacy.**
- Điểm neo: GuiEvent::ActivityUpdated; producer FetchLastOnline và apply_metadata_impact trong src/gui/worker.rs; handler trong src/gui.rs; header trong src/gui/workspace.rs.
- Fencing phải được kiểm tra trước mọi ghi presence_map: logged-in, owner, runtime. Cập nhật tất cả producer, kể cả push; runtime lấy từ session tạo tác vụ, không từ session hiện tại lúc gửi response. Worker phải kiểm tra command thuộc session đang xử lý.
- Header/dot/text phải dùng cùng privacy rule: show_online chưa biết hoặc false thì không hiển thị LastActivity cache. PrivacyUnavailable/Unavailable không được biến thành online/offline suy đoán. Khi quyền bị invalidate/thay đổi, cache cũ không được tiếp tục hiện qua đường render khác.
- Không tự thêm polling hay request protocol mới. Giữ cách tra cứu on-demand hiện có và xử lý thay đổi phạm vi khi response tới trễ.

**Bước 5 — regression bắt đúng lỗi, không chỉ test helper/predicate.**
Dùng barrier/channel và event handler/worker boundary thật; không sleep để cầu may. Với mỗi regression, xác nhận nó bắt được hành vi cũ khi khả thi và ghi rõ phương pháp. Không hạ assertion, thêm ignore hay biến lỗi thành success.

| Nhóm | Ca bắt buộc |
| --- | --- |
| Metadata | Snapshot cũ bắt đầu trước mutation, trả sau ACK: không đảo trạng thái labels/manual-unread/archive và không persist dữ liệu cũ; kiểm tra cả event riêng và AccountMetadata. |
| Metadata | Snapshot thành công hoặc lỗi tới khi mutation đang pending: không xóa pending/uncertain của thao tác mới. |
| Metadata | Snapshot đầu tiên hợp lệ được áp dụng; read mới sau mutation vẫn cập nhật; hai response về ngược thứ tự chỉ giữ bản hợp lệ mới. |
| Metadata | Owner khác, runtime cũ cùng UID, logout giữa read và delivery: bỏ response. |
| Preflight | Mở preflight -> logout/session expiry/profile switch: state bị xóa, stop monitor được quan sát, không StartCall. |
| Preflight | Relogin cùng UID/runtime khác; confirm muộn hoặc command đã enqueue rồi đổi account: worker từ chối. |
| Preflight | Scope đúng: xác nhận phát đúng một lệnh tới đúng người; Cancel/Escape dừng monitor; không thiết bị thật. |
| Presence | Sai owner/runtime hoặc logged-out: không ghi cache; kiểm tra cả HTTP và push producer. |
| Presence | Có cache LastActivity nhưng privacy false/unknown/invalidated: không dot/text online; privacy bật + dữ liệu hợp lệ vẫn hiện. |
| Presence | PrivacyUnavailable/Unavailable và response cũ đến sau đổi privacy không làm lộ trạng thái. |

Đặc biệt: test test_phase4_initial_boot_sync_generation_zero_accepted hiện chỉ khẳng định bỏ rào chắn; thay bằng các ca hydrate hợp lệ và late snapshot/mutation nói trên. Không giữ test sai làm tiêu chí thiết kế.

**Bước 6 — kiểm chứng và hai lượt audit sạch.**
1. Chạy focused regression sau mỗi nhóm sửa. Khi nguồn ổn định, chạy cargo test --locked --lib -j1; cargo test --locked --test desktop_backup_v2 -j1; cargo clippy --locked --all-targets -j1 -- -D warnings; cargo fmt --all -- --check; git diff --check. Thiết lập PATH tới toolchain local, CMAKE_POLICY_VERSION_MINIMUM=3.5 nếu cần và CTDOZ_DATA_DIR vào thư mục tạm. Không dùng session/database thật trong test.
2. Chạy lại module startup/config song song ít nhất 100 vòng với 8 hoặc 16 thread; thêm các regression mới vào nhóm stress thích hợp. Không ép toàn suite một thread để giấu race.
3. Audit lượt 1 theo luồng dữ liệu và account boundaries; lượt 2 theo interleaving/lỗi/cancellation. Khóa candidate bằng commit + source hash hoặc fingerprint working tree; nguồn phải giống nhau trong hai lượt. Bất cứ phát hiện nào hoặc sửa source/test nào đều đặt bộ đếm sạch về 0 rồi audit lại bản mới. Không tự xưng hai reviewer độc lập nếu chỉ một agent thực hiện.
4. Chỉ ghi kết quả đã chạy, gồm failed/ignored, lệnh, snapshot và giới hạn. Nếu check lỗi do thay đổi khác, phân biệt rõ, không tự sửa rộng hoặc tuyên bố toàn bộ pass. Không ghi cứng số test kỳ vọng.

**Ràng buộc bảo toàn.** Giữ cache hội thoại trước Ready, merge không đảo recall/local delete/plaintext mới; timeout AUTO theo origin; gate Banner/Settings/V1; ACK chống gửi lặp; authority config; runtime cipher isolation; auto-ZIP và phân loại error 204; import backup batch/write gate. Không sửa protocol, schema DB, entitlement hay routing để chữa lỗi UI/revision. Không reset/clean/revert shared tree, commit/push/tag/release hoặc gửi tin/gọi thử thật. Comments English; chuỗi người dùng tiếng Việt; không log secret.

**Bàn giao.** Cập nhật kế hoạch này và docs/research/2026-11-23-sync-config-audit-dossier.md; cập nhật verification spec liên quan nếu thay bằng chứng, giữ native tasks mở. Báo bảng: finding -> thay đổi -> regression -> kết quả thực chạy; các file thay đổi; hash bản đã audit; hai lượt audit và giới hạn. ZIP-08, sync điện thoại thật, ChangeDevice/QueueEvict và native call/privacy acceptance không được đánh dấu xong bằng unit test. Codex sẽ audit lại sau; không dùng nhãn Phase 100% thay cho bằng chứng.

---

### Kế hoạch hành động chi tiết khắc phục ba hồi quy audit (Antigravity Remediation Plan)

#### 1. Sơ đồ luồng dữ liệu (Producer -> WorkerCmd -> Worker -> GuiEvent -> State -> Render)

```text
A. METADATA REVISION & MUTATION FENCING:
[UI Action: Add/Del Label, Mark Unread, Archive]
       |
       +--> Sinh monotonic token: rev = crate::search::generation()
       +--> Cập nhật UI State: self.{domain}_generation = rev; self.{domain}_revision = rev; self.{domain}_pending = true;
       +--> Gửi WorkerCmd::{Labels|ManualMark|Archive} { owner, runtime_id, generation: rev }
                  |
            (Worker Tokio) ---> HTTP POST /api/...
                  |                   | (nếu có lỗi / thành công)
                  v                   v
            GuiEvent::{Labels|ManualMarks|Archive} { owner, runtime_id, generation: rev, mutation: true }
                  |
            (Gui Event Loop)
                  +--> Kiểm tra: is_logged_in && owner == user_uin && runtime_id == account_runtime_id
                  +--> Vì mutation == true: Yêu cầu generation == self.{domain}_generation
                  +--> Cập nhật state, giải phóng pending = false, áp dụng DB/UI, toast (nếu có)

[Worker Background Refresh / Periodic Poll / Cache Restore / AccountMetadata]
       |
       +--> Lấy timestamp phiên bản snapshot TRƯỚC await: snapshot_rev = crate::search::generation()
       +--> Await HTTP GET / SQLite read
       +--> Gửi GuiEvent::{Labels|ManualMarks|Archive} { owner, runtime_id, generation: snapshot_rev, mutation: false }
            HOẶC GuiEvent::AccountMetadata { owner, runtime_id, revision: snapshot_rev, labels, archive, ... }
                  |
            (Gui Event Loop)
                  +--> Kiểm tra: is_logged_in && owner == user_uin && runtime_id == account_runtime_id
                  +--> Vì mutation == false:
                       - Kiểm tra: !self.{domain}_pending && snapshot_rev >= self.{domain}_revision
                       - NẾU THỎA MÃN: Cập nhật self.{domain}_revision = snapshot_rev, nạp dữ liệu.
                       - NẾU KHÔNG THỎA (mutation đang pending hoặc snapshot_rev < domain_revision):
                         HỦY BỎ! Tuyệt đối không xóa pending, không đổi uncertain, không đè state/DB.

B. PREFLIGHT & MICROPHONE LIFECYCLE:
[User Click Call Button in Header/Profile]
       |
       +--> Kiểm tra: self.is_logged_in && !self.user_uin.is_empty()
       +--> Khởi tạo PreflightCallState { owner: user_uin, runtime_id: account_runtime_id, ... }
       +--> Khởi động PreflightMicMonitor (thực tế hoặc test seam)
       +--> Modal Render Gate: p.owner == self.user_uin && p.runtime_id == self.account_runtime_id && self.is_logged_in
                  |
       [User Confirms Call]
                  +--> Gửi WorkerCmd::StartCall { owner: p.owner, runtime_id: p.runtime_id, target, is_video }
                  |
            (Worker Tokio StartCall Handler)
                  +--> Tra cứu session hiện thời s:
                       Kiểm tra s.is_active() && s.account_key() == owner && s.runtime_id() == runtime_id
                       NẾU KHÔNG KHỚP: Hủy command ngay lập tức, ghi trace log, không gửi socket/VoIP frame!
[Logout / Expiry / Profile Switch / Account Runtime Change]
       |
       +--> Gọi self.clear_preflight_call():
            - Drop PreflightCallState
            - Gọi monitor.stop() (dừng audio thread an toàn, non-blocking UI)
            - Đảm bảo khi login tài khoản mới, dialog cũ không thể hiển thị lại.

C. PRESENCE CACHE & PRIVACY FENCING:
[FetchLastOnline Cmd OR WebSocket apply_metadata_impact Push]
       |
       +--> Producer gắn session runtime_id tại thời điểm khởi tạo tác vụ (từ session active lúc nhận push)
       +--> Gửi GuiEvent::ActivityUpdated { owner, runtime_id, uid, activity }
                  |
            (Gui Event Loop)
                  +--> Fencing check: self.is_logged_in && owner == self.user_uin && runtime_id == self.account_runtime_id
                  +--> Nếu hợp lệ: cập nhật self.presence_map.insert(uid, activity)
                  +--> Nếu không hợp lệ: DROP silently!
[Workspace Header / Subtitle / Online Dot Rendering]
       |
       +--> Tra cứu inbox_policy.show_online:
            Gate nghiêm ngặt: let show_online = self.inbox_policy.show_online == Some(true);
            - NẾU !show_online (Unknown/None hoặc Some(false)):
              KHÔNG vẽ online green dot! KHÔNG hiển thị LastActivity format ("Truy cập X phút trước").
              Chỉ hiển thị nhãn bạn bè/người lạ bình thường.
            - NẾU show_online == true:
              Chỉ vẽ dot và format time khi activity là Online hoặc LastOnline hợp lệ.
              Tuyệt đối không suy diễn PrivacyUnavailable / Unavailable thành online hay offline.
```

#### 2. Bảng Invariant / Lỗ hổng / Giải pháp / Test Matrix

| Hồi quy / Phát hiện | Invariant bắt buộc | Lỗ hổng hiện tại | Giải pháp kiến trúc | Regression Test bắt buộc |
| :--- | :--- | :--- | :--- | :--- |
| **1. Metadata Snapshot Overwrite** | Background snapshot không bao giờ được ghi đè UI mutation mới hơn, không xóa pending, không đổi uncertain. | `generation = 0` được coi là wildcard vượt qua mọi kiểm tra phiên bản. `AccountMetadata` và cache restore không kiểm tra generation/revision. | Thêm `revision: u64` vào `AccountMetadata`. Snapshot gán `snapshot_rev = search::generation()` trước await. Tách cờ `mutation: bool` trong `GuiEvent::{Labels,ManualMarks,Archive}`. Snapshot nền yêu cầu `!pending && snapshot_rev >= domain_revision`. Mutation ACK yêu cầu `generation == domain_generation`. | `test_metadata_stale_snapshot_arriving_after_ack_does_not_overwrite_state`<br>`test_metadata_snapshot_while_mutation_pending_does_not_clear_pending_or_uncertain`<br>`test_metadata_hydrate_valid_and_out_of_order_responses_keep_latest_only`<br>`test_metadata_stale_runtime_or_mismatched_owner_dropped` |
| **2. Preflight / Mic Lifecycle** | Preflight call và microphone monitor phải bị cô lập chặt chẽ theo account scope; không bao giờ sống sót qua logout/expiry/profile switch; không gửi call qua session khác. | `PreflightCallState` không có `owner` và `runtime_id`. `clear_presence_state` không drop preflight/stop mic. `WorkerCmd::StartCall` không kiểm tra account boundary. Test gọi monitor thật chạm thiết bị âm thanh. | Gắn `owner` và `runtime_id` vào `PreflightCallState`. Dọn dẹp trong `clear_preflight_call()`. Thêm `owner, runtime_id` vào `WorkerCmd::StartCall` và xác thực tại worker. Cung cấp test seam/mock cho `PreflightMicMonitor` không chạm audio device. | `test_preflight_logout_expiry_clears_state_and_stops_monitor`<br>`test_preflight_relogin_different_runtime_worker_rejects_old_call_cmd`<br>`test_preflight_scope_correct_single_call_and_cancel_stops_monitor_without_live_devices` |
| **3. Presence Cache & Privacy** | Presence cache chỉ được ghi cho đúng session active; header chỉ được hiển thị online/last activity khi privacy gate cho phép rõ ràng (`show_online == Some(true)`). | `GuiEvent::ActivityUpdated` thiếu `owner` / `runtime_id`. UI ghi `presence_map` mà không kiểm tra session boundary. Header render cache bỏ qua `show_online == false/None`. `PrivacyUnavailable` có thể bị rò rỉ. | Bổ sung `owner, runtime_id` vào `GuiEvent::ActivityUpdated`. Worker gắn runtime lúc tạo tác vụ. Handler UI kiểm tra fencing trước khi insert `presence_map`. Header áp dụng cổng cứng `show_online == Some(true)`. | `test_presence_mismatched_owner_runtime_or_logged_out_does_not_write_cache`<br>`test_presence_cached_last_activity_hidden_when_privacy_false_or_unknown`<br>`test_presence_privacy_unavailable_does_not_leak_status` |

#### 3. Danh sách File dự kiến thay đổi

1. `src/gui/worker.rs`:
   - Thêm `runtime_id: usize` vào `WorkerCmd::Labels`, `WorkerCmd::ManualMark`, `WorkerCmd::Archive`.
   - Thêm `owner: String, runtime_id: usize` vào `WorkerCmd::StartCall`, `WorkerCmd::FetchLastOnline`.
   - Bổ sung `snapshot_rev` trước khi `join!` trong metadata refresh loop và trước khi đọc `account_metadata`.
   - Bổ sung `runtime_id` trong `apply_metadata_impact` WebSocket push dispatch.
   - Xác thực `owner` & `runtime_id` trong handler `WorkerCmd::StartCall` và `WorkerCmd::FetchLastOnline`.
2. `src/gui.rs`:
   - Thêm `runtime_id: usize, mutation: bool` vào `GuiEvent::Labels`, `GuiEvent::ManualMarks`, `GuiEvent::Archive`.
   - Thêm `revision: u64` vào `GuiEvent::AccountMetadata`.
   - Thêm `owner: String, runtime_id: usize` vào `GuiEvent::ActivityUpdated`.
   - Thêm các trường `labels_revision: u64, manual_mark_revision: u64, archive_revision: u64, privacy_revision: u64` vào `ZaloGuiApp`.
   - Cập nhật handlers cho `GuiEvent::Labels`, `ManualMarks`, `Archive`, `AccountMetadata`, `ActivityUpdated` với đầy đủ revision fencing logic.
   - Thêm `owner: String, runtime_id: usize` vào `PreflightCallState`.
   - Cập nhật `open_preflight_call` và thêm `clear_preflight_call(&mut self)`. Gọi trong `clear_presence_state()`, logout, session expiry, profile switch.
3. `src/gui/labels_panel.rs`:
   - Cập nhật các caller phát sinh `WorkerCmd::Labels` để truyền `self.account_runtime_id`.
4. `src/gui/modals.rs`:
   - Trong `render_preflight_dialog`: kiểm tra `p.owner == self.user_uin && p.runtime_id == self.account_runtime_id && self.is_logged_in`.
   - Khi bấm Xác nhận gọi: gửi `WorkerCmd::StartCall { owner: p.owner.clone(), runtime_id: p.runtime_id, ... }`.
5. `src/media.rs`:
   - Thêm mock constructor hoặc stop channel an toàn cho `PreflightMicMonitor` để test không chạm thiết bị âm thanh.
6. `src/gui/workspace.rs`:
   - Cập nhật caller `WorkerCmd::StartCall` và `WorkerCmd::FetchLastOnline`.
   - Cập nhật logic render subtitle và online dot trong header: chỉ cho phép hiển thị khi `self.inbox_policy.show_online == Some(true)`.
   - Cập nhật và bổ sung toàn bộ các regression tests theo đúng ma trận yêu cầu.

#### 4. Tradeoffs & Đường lỗi (Failure Paths Analysis)

- **Tradeoff về Monotonic Revisions**: Tái sử dụng `crate::search::generation()`. Đây là `AtomicU64` toàn cục tăng dần đơn điệu (`Ordering::SeqCst`). Việc dùng nó trước await HTTP bảo đảm mọi snapshot bắt đầu trước một mutation đều có `snapshot_rev < mutation_rev`, ngăn chặn triệt để hiện tượng ghi đè ngoài ý muốn khi response về chậm.
- **Xử lý Response Lỗi (Error Path)**:
  - Khi snapshot nền gặp HTTP error (timeout, 5xx): worker phát event với lỗi, nhưng GUI kiểm tra nếu `mutation == false` thì TUYỆT ĐỐI KHÔNG gán cờ `uncertain = true` và KHÔNG hiển thị toast lỗi làm hoang mang người dùng.
  - Khi mutation của người dùng gặp lỗi: vì `generation == domain_generation`, GUI xử lý hoàn tác, xóa pending, gán `uncertain = true` và hiển thị toast cảnh báo chính xác.
- **Race Condition giữa Enqueue Call và Account Switch**:
  - Nếu người dùng bấm Gọi, `WorkerCmd::StartCall` được đẩy vào Tokio channel, ngay khoảnh khắc đó phiên bị hết hạn hoặc chuyển tài khoản: Worker pop command ra, kiểm tra `s.is_active() && s.account_key() == owner && s.runtime_id() == runtime_id`. Thấy runtime/owner lệch nhau, Worker hủy ngay command, không phát bất kỳ gói tin VoIP nào lên Zalo server.

---

### Kết quả Thực thi và Kiểm chứng Thực tế (24/11/2026)

Toàn bộ 3 phát hiện hồi quy audit đã được sửa đổi và kiểm chứng toàn diện trên nhánh làm việc hiện tại:

1. **Compilation, Formatting & Lints:**
   - `cargo check --locked --all-targets`: **PASS** (0 lỗi).
   - `cargo clippy --locked --all-targets -j1 -- -D warnings`: **PASS** (0 cảnh báo).
   - `cargo fmt --all -- --check`: **PASS** (sạch định dạng).
   - `git diff --check`: **PASS** (0 conflict marker, 0 trailing whitespace).

2. **Unit & Regression Suites:**
   - `cargo test --locked --lib gui::workspace::tests::test_metadata`: **4 passed; 0 failed**.
   - `cargo test --locked --lib gui::workspace::tests::test_preflight`: **3 passed; 0 failed**.
   - `cargo test --locked --lib gui::workspace::tests::test_presence`: **3 passed; 0 failed**.
   - `cargo test --locked --lib ringtone_and_preflight`: **1 passed; 0 failed**.
   - `cargo test --locked --lib preflight_call_initializes_properly`: **1 passed; 0 failed**.
   - `cargo test --locked --lib -j1`: **1144 passed; 0 failed; 11 ignored** (70.57s).

3. **Integration & Stress Suites:**
   - `cargo test --locked --test desktop_backup_v2 -j1`: **4 passed; 0 failed; 0 ignored** (45.02s).
   - `cargo test --locked --lib gui::startup_and_server_config_tests -- --test-threads=8` (10 vòng liên tiếp): **16/16 passed** mỗi vòng (tổng 160 lần chạy, không flakiness hay race condition).



## Codex trực tiếp sửa vòng audit tiếp theo — 24/11/2026

Phạm vi: ba finding còn lại trên working tree c9a274f; không commit/push/reset. Tách kết quả metadata thành Snapshot/Read/Mutation để UI read khớp request giải phóng pending nhưng snapshot nền không giải phóng request khác. Giữ owner/runtime xuyên qua CallBridge/CallEngineCmd, kiểm tra trước và sau readiness và dùng snapshot đã xác nhận. Tăng privacy revision tại invalidation và dispatch UI để snapshot cũ không mở lại gate.

Regression: command UI thật -> mock transport -> event handler cho read thành công/lỗi ở cả ba domain; invalidation -> stale/fresh AccountMetadata; privacy UI dispatch/ACK -> stale metadata; hàng đợi call thật + readiness barrier + thay phiên trước/sau dequeue, error sau thay phiên và scope hợp lệ. Không chạm thiết bị/network thật. Sau sửa khóa source fingerprint, chạy hai lượt audit khác phương pháp trên cùng nguồn; bất cứ finding hoặc chỉnh sửa source/test nào đặt lại bộ đếm về 0. Kết quả và giới hạn sẽ ghi ở dossier, không dùng các nhãn PASS/100% cũ làm bằng chứng.


### Kết quả Codex trực tiếp sửa và hội tụ cuối — 24/11/2026

Thay thế các kết luận PASS/100% cũ đối với finding còn lại. Bản cuối tách MetadataResultKind::{Snapshot,Read,Mutation}; giữ scope qua CallEngineCmd; dùng producer-stamped invalidation + runtime và cutoff request privacy; cập nhật policy ở ACK/background và contact drawer. Source review còn tìm và sửa read/mutation overlap ở ManualMark qua shared admission semaphore.

Các candidate trung gian bị loại sau finding dù đã qua test; không cộng dồn lượt sạch. Candidate cuối trên main/HEAD c9a274f: `c494c88d95052d3dfd8f1a8d9a9c13b6a184aa524962e197a606a08cbade17f3` (240 tệp nguồn/test/build). Hai lượt audit liên tiếp sạch, giữ nguyên nguồn giữa hai lượt: lượt 1 dataflow/ownership + library 1154 passed/11 ignored; lượt 2 interleaving/error/runtime + 100 vòng stress 8/16 thread, 3000 test executions đạt, Clippy all-targets -D warnings exit 0 và desktop_backup_v2 4/4 đạt.

Bằng chứng chi tiết, red/green, các lần reset bộ đếm và giới hạn nằm trong [audit dossier](2026-11-23-sync-config-audit-dossier.md). Hai lượt do cùng Codex, không phải hai reviewer độc lập. Không commit/push/release. Native acceptance và các mục AMBIGUOUS vẫn mở.
