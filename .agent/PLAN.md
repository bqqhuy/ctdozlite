# .agent/PLAN.md - Kế hoạch Thực thi Codex Parity & Settings UI/UX (B1 -> B5)

> [!NOTE]
> **LỊCH SỬ CŨ (HISTORICAL / SUPERSEDED)**: Chu kỳ audit trước đó về Startup Readiness, Sync Banner & Hidden Chat đã đạt 2 lượt APPROVE liên tiếp (Lượt 3 & Lượt 4) vào ngày 24/11/2026. Lịch sử được bảo lưu nguyên vẹn tại [REVIEW_LOG.md](file:///c:/Projects/ZaloLite/.agent/REVIEW_LOG.md).
> Phần dưới đây là kế hoạch thực thi mới theo tài liệu chính thức: `docs/research/2026-11-24-codex-ui-ux-settings-implementation-plan.md`.

---

## 1. Mục tiêu & Nguyên tắc Cốt lõi
- **Nguồn chuẩn**: `AGENTS.md`, `GLOSSARY.md`, `docs/standards/ui-ux-consistency.md`, `docs/research/2026-11-24-codex-ui-ux-settings-implementation-plan.md`.
- **Ràng buộc cứng**:
  - Không commit, không push, không tag, không release trong phiên này.
  - Không reset, clean, stash hoặc xóa các tệp untracked.
  - Không xóa `tools_panel.rs` hoặc `self.tools` trước khi hoàn tất migration end-to-end từng chức năng.
  - Giữ nguyên KDF PBKDF2 cho PIN (không downgrade SHA-256).
  - Không tạo UI stubs; mọi control tương tác phải gắn liền với backend và reactive state.

---

## 2. Bảng Đối chiếu UXS-00 .. UXS-08 (Mục tiêu, Gap thực & Hợp đồng Kỹ thuật)

| Gói | Yêu cầu (Requirement) | Hành vi hiện tại (Current Behavior) | Gap thực tế (Actual Gap) | Bằng chứng (Evidence) | Luồng lệnh & Dữ liệu (Entry -> Backend -> Event -> Persistence) | Giải pháp đề xuất (Proposed Patch) | Kiểm chứng (Verification) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **UXS-00** | Inventory & Khóa hợp đồng | Đã có 10 `SettingsTab` trong `modals.rs`; `ToolsPanel` có 10 tab con trong `tools_panel.rs` | Chưa khóa bản đồ di chuyển từng tính năng | `src/gui/modals.rs:804-814`, `src/gui/tools_panel.rs:5` | Khảo sát tĩnh `file:line`, kiểm kê struct thật | Lập bản đồ phụ thuộc và khoanh vùng lease | SCOUT inventory report |
| **UXS-01** | Settings host, modal & điều hướng | Khi `win_w < 700.0` (compact), sidebar bị ẩn hoàn toàn, thay bằng `ComboBox` dễ gây nhầm lẫn; vùng nội dung thiếu scroll dọc đồng nhất | Compact mode mất thanh icon điều hướng; nội dung có thể tràn/cắt xén trên màn hình chiều cao nhỏ (<560px) | `src/gui/modals.rs:756, 785, 900-915` | `render_settings_modal` -> egui `Window` -> `egui::ScrollArea::vertical` | Thích ứng sidebar 48px icon-only khi compact; bọc `ScrollArea::vertical` cho toàn bộ panel nội dung và sidebar | Test resize viewport 640x480, 800x600, 1000x800 |
| **UXS-02** | Di chuyển chức năng khỏi Tools | Tab `DataManagement` trong Settings chứa 2 nút đóng modal và mở `tools_panel.rs` tab 1 & 8 | Người dùng bị đẩy sang popup Tiện ích thay vì thao tác trực tiếp trong Settings; vi phạm Zalo PC parity | `src/gui/modals.rs:1421-1430`, `src/gui/tools_panel.rs:251-290` | UI click -> `self.tools_command(ToolsCommand::ExportHistory / Import)` -> `WorkerCmd::Tools` -> `GuiEvent::Tools` -> `self.tools.status` | Nhúng trực tiếp control Backup (Xuất SQLite) và Import (chọn file/thư mục + checkbox xác nhận) vào `SettingsTab::DataManagement`; giữ nút mở Kho Media & zCloud (chưa migrate tab 8) | Unit test & regression test backup/import workflow |
| **UXS-03** | Hoàn thiện từng nhóm Settings | Các tab General, Security, Sync, DataManagement, Appearance, Messages có renderer cơ bản | Một số cài đặt chưa phản ánh đúng provenance hoặc chưa gắn kết với consumer thực tế | `src/gui/modals.rs:969-1850` | `AppSettings` -> SQLite `tool_config` / local preferences | Tinh chỉnh từng nhóm theo backend thực tế, không thêm toggle giả lập | Test lưu và nạp lại cấu hình sau restart |
| **UXS-04** | Startup readiness & Config consumer | Startup preload đã có baseline timeout 9s, lọc deleted/recalled và phân loại status | Phân loại `Offline` dựa trên chuỗi có thể bắt trượt lỗi `reqwest::Error` dạng `"error sending request..."` hoặc `RequestDeadlineElapsed` | `src/gui/worker.rs:2129-2136` | `fetch_baseline_startup_threads_reported` -> `reqwest::Error::is_connect / is_timeout` | Downcast `reqwest::Error` & `RequestDeadlineElapsed` để phân loại `Offline` chính xác; tối ưu lệnh `RefreshThreads` sau startup | Test cold cache, offline simulate, timeout |
| **UXS-05** | Composer, file card, timeline | Composer đã có preview attachment, đo width, giữ extension; bubble tin nhắn có MQA toolbar | Cần rà soát tính toàn vẹn khi nhiều tệp, draft restore và clear | `src/gui/workspace.rs:2800-3400` | Composer state -> Draft storage -> Attachment pipeline | Kiểm tra và giữ vững các bất biến draft/caption và file card | Unit test composer attachment measuring |
| **UXS-06** | Emoji màu & Tính đúng của text | egui 0.29.1 render glyph font hệ thống | Cần kiểm tra hiển thị Unicode emoji sequences (ZWJ, skin tone) | `src/gui/theme.rs`, `epaint` font definitions | Font loading -> Glyph rasterizer | Rà soát rendering Unicode sequences trên nền tảng Windows | Kiểm chứng trực quan và test text shaping |
| **UXS-07** | Media lifecycle & Storage cleanup | Quản lý cache và preview micro/audio đã có token owner/runtime | Cần đảm bảo giải phóng thiết bị micro ngay khi đóng modal/tab switch | `src/gui/modals.rs:2658`, `src/media.rs` | `PreflightMicMonitor::stop` -> CPAL device release | Đảm bảo stop idempotent khi đổi tài khoản hoặc đóng modal | Test preflight monitor cleanup |
| **UXS-08** | Tích hợp & Nghiệm thu | Hệ thống đã có 1173 tests passed, hard gates prepush sạch | Cần kiểm chứng candidate mới qua chu trình review kép độc lập B4 | Toàn bộ codebase | `cargo test`, `cargo clippy`, `audit-prepush.ps1` | Chạy Cổng B3 và chu trình Dual Review 2 lượt sạch | Automated gates + Biên bản APPROVE kép |

---

## 3. Ba Patch Đầu Tiên (Triển khai Trọng tâm — Đã Phê Duyệt Sau Thẩm Định Advisor)

### Patch P1: Settings Host, Modal & Adaptive Navigation
- **Tệp được phép sửa**: `src/gui/modals.rs`, `src/gui.rs` (bổ sung test viewport matrix).
- **Mục tiêu & Điều kiện chốt**:
  1. Thích ứng sidebar: Khi `compact = (win_w < 700.0)`, hiển thị sidebar thu gọn `sidebar_w = 48.0` icon-only có tooltip tiếng Việt đầy đủ qua `crate::i18n::tr`; khi `win_w >= 700.0`, hiển thị sidebar `sidebar_w = 200.0` (icon + title).
  2. Tính toán chính xác outer width sidebar: khi compact, sidebar outer width đúng 48px; `content_w = (win_w - sidebar_w - 48.0).max(64.0)` đảm bảo tổng không vượt quá `win_w` và không cắt nút `X`.
  3. Khi compact: ẩn heading `"Cài đặt"`, căn giữa icon tại `rect.center()`, luôn hiển thị tiêu đề tab đầy đủ `ui.label(tab_title)` ở header bên phải để tránh việc chỉ biết tab qua hover tooltip.
  4. Bọc `egui::ScrollArea::vertical().auto_shrink([false, false])` cho **cả sidebar** (khi `win_h < 450px`) lẫn **panel nội dung bên phải** (với `.max_height((win_h - 100.0).max(32.0))`).
  5. Reflow các layout cố định (`Notifications`, `Appearance`, `General`, `Calls`) để hiển thị gọn gàng khi `content_w` hẹp mà không bị scroll ngang.
  6. Bổ sung `SettingsTab::DataManagement` và `SettingsTab::Calls` vào test viewport matrix `settings_tabs_fit_small_wide_and_high_dpi_viewports`.

### Patch P2: Migration Hoàn chỉnh — Chuyển Backup / Import / Export sang `SettingsTab::DataManagement`
- **Tệp được phép sửa**: `src/gui/modals.rs`, `src/gui/tools_panel.rs` (mở visibility `pub(super) fn render_tools_import`).
- **Mục tiêu & Điều kiện chốt**:
  1. **Bắt buộc giữ lại nút**: `"Mở kho Media và zCloud"` (`modals.rs:1426`) vì tab 8 Media/zCloud chưa được migrate ở patch này.
  2. Thay thế nút `"Mở công cụ nhập / xuất dữ liệu"` (`modals.rs:1421`) bằng cụm điều khiển trực tiếp:
     - Nút "Sao lưu ngay (Xuất SQLite)": gọi `self.tools_command(ToolsCommand::ExportHistory)`.
     - Checkbox xác nhận `self.tools.import_owned`: "Tôi xác nhận dữ liệu nguồn thuộc tài khoản đang đăng nhập".
     - Nút "Chọn tập tin": gọi `self.tools_command(ToolsCommand::Import { directory: false, confirmed: true })`.
     - Nút "Chọn thư mục": gọi `self.tools_command(ToolsCommand::Import { directory: true, confirmed: true })`.
     - Hiển thị nhãn trạng thái `self.tools.status` trực tiếp bên dưới các nút điều khiển.
  3. Trong `SettingsTab::Sync` (dòng 1312), chuyển hướng nút `"Nhập / Xuất lịch sử…"` chuyển thẳng sang `self.settings_tab = SettingsTab::DataManagement` thay vì đóng modal mở `Tools`.

### Patch P3: Chuẩn hóa Phân loại Lỗi Mạng & Startup Readiness
- **Tệp được phép sửa**: `src/gui/worker.rs`.
- **Mục tiêu & Điều kiện chốt**:
  1. Trong `fetch_baseline_startup_threads_reported`: Duyệt `e.chain()` kiểm tra:
     - `cause.downcast_ref::<reqwest::Error>().is_some_and(|re| re.is_connect() || re.is_timeout())`
     - `cause.downcast_ref::<crate::session::RequestDeadlineElapsed>().is_some()`
     - Kết hợp kiểm tra lowercase message (`connect`, `timed out`, `timeout`, `dns`, `tcp`, `network`).
     - Giữ nguyên lỗi HTTP (500), Envelope error, decode JSON, auth error ở trạng thái `BaselineFetchStatus::Failed`.
  2. Trì hoãn việc gửi `WorkerCmd::RefreshThreads` trong worker khi `run_startup_preload` đang chạy dở bước 1–3 để không chạy tranh chấp với cold baseline.
