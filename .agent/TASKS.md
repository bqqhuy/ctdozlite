# .agent/TASKS.md - Danh sách Nhiệm vụ & Trạng thái Thực thi (Codex Implementation Plan)

> [!NOTE]
> **LỊCH SỬ CŨ (HISTORICAL / SUPERSEDED)**: Các nhiệm vụ T-01 đến T-10 thuộc đợt audit trước đã hoàn tất và đạt 2 lượt APPROVE liên tiếp vào ngày 24/11/2026. Lịch sử được lưu tại [REVIEW_LOG.md](file:///c:/Projects/ZaloLite/.agent/REVIEW_LOG.md).
> Dưới đây là danh sách công việc cho đợt triển khai phương án Codex:

---

## Bảng Trạng thái Công việc (Job Status Table)

| Job ID | Task / Patch | Vai trò | Model thực tế | File được cấp quyền (Lease) | Mục tiêu | Trạng thái |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **J-00** | UXS-00 | SCOUT / Orchestrator | `flash_lite` & `gemini-4-carbon-high` | Read-only | Khảo sát `modals.rs`, `tools_panel.rs`, lập bảng inventory | **DONE** |
| **J-01** | Patch P1 (UXS-01) | WORKER_LOGIC | `gemini-4-hydrogen-medium` | `src/gui/modals.rs`, `src/gui.rs`, `src/gui/theme.rs` | Thích ứng sidebar compact 48px, menu dropdown có chữ khi compact, bọc `ScrollArea::vertical` nội dung | **DONE** |
| **J-02** | Patch P2 (UXS-02) | WORKER_LOGIC | `gemini-4-hydrogen-medium` | `src/gui/modals.rs`, `src/gui/tools_panel.rs`, `src/gui.rs` | Di chuyển thao tác Backup / Import / Export sang `SettingsTab::DataManagement` thật; single-flight | **DONE** |
| **J-03** | Patch P3 (UXS-04) | WORKER_LOGIC | `gemini-4-hydrogen-medium` | `src/gui/worker.rs` | Downcast `reqwest::Error` & `RequestDeadlineElapsed` phân loại `Offline`; tối ưu `RefreshThreads` | **DONE** |
| **J-04** | Gate B3 | Orchestrator | `gemini-4-carbon-high` | Read-only | Chạy fmt, check, clippy, unit test lib 1173 passed, và `audit-prepush.ps1` hard gates pass | **DONE** |
| **J-05** | Dual Review B4 | REVIEWER_A & B | `gemini-4-carbon-high` & `claude-opus-5-5-high` | Read-only | Rà soát độc lập 2 lượt liên tiếp sạch lỗi (APPROVE kép Lượt 2 & Lượt 3) | **DONE** |
| **J-06** | Handoff B5 | Orchestrator | `gemini-4-carbon-high` | `.agent/`, `specs/` | Lập biên bản nghiệm thu, tổng hợp hồ sơ sẵn sàng cho Codex audit | **DONE** |
| **J-07** | Startup & Cache Parity Remediation | WORKER_LOGIC | `gemini-4-hydrogen-medium` | `src/gui/worker.rs`, `src/gui.rs`, `src/gui/workspace.rs` | Tách kết quả per-source (`SourceFetchStatus`), cache usability cấu trúc, preview độc lập danh bạ, fenced backfill, 6 controlled mock pipeline tests | **DONE** |
| **J-08** | Automated Gate Verification | Orchestrator | `gemini-4-carbon-high` | Read-only | fmt, check, clippy (0 warnings), `cargo test --lib` 1178 passed (11 ignored), prepush audit hard gates passed | **DONE** |


## Codex remediation — 25/11/2026 (không kế thừa APPROVE của candidate cũ)

- Phạm vi: startup/cache/backfill và pending data operation của Settings.
- Chi tiết finding/candidate/checks: `docs/research/2026-11-25-startup-settings-remediation.md`.
- Trạng thái: ACCEPTED_SCOPED (CODE/FIXTURE): 1185 library passed/11 ignored,
  backup 4 passed, stress 200 passed, Clippy/fmt/diff-check PASS; hai audit cùng Codex
  trên cùng fingerprint, không phải review kép cross-model. Native vẫn pending.
- Giữ nguyên task native và UXS chưa triển khai; không commit/push/release.


## Codex combined session/startup — 25/11/2026

ACCEPTED_SCOPED (CODE/FIXTURE), candidate
`271610c1f91ad9859bc782830fe683cbca7b86650d2afe7b7433956fbae3db87`.
Hồ sơ: `docs/research/2026-11-25-session-startup-combined-audit.md`.
1192 library passed/11 ignored; 260 stress passed; storage/backup integration 6 passed.
Clippy all-targets -D warnings, fmt, diff-check PASS. Integration LNK4199 DELAYLOAD
warning được ghi nhận trong hồ sơ. Hai lượt scoped sạch cùng Codex trên cùng source;
không phải review kép cross-model, không cộng candidate cũ. Native GUI/phone/Web và
các UXS còn mở vẫn pending. Không commit/merge/push/tag/release.
