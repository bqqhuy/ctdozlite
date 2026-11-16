# Tracker triển khai — Các phát hiện F01–F21

**Trạng thái:** đã triển khai đủ P1, P2 và P3 ở mức code/fixture. Hai Pro auditor review độc lập; sau vòng sửa R1 và R2 cả hai **APPROVE**. Đã merge vào `main` (local và GitHub).
**Mốc audit lịch sử:** `main` tại `f0d3206e875d083c7f8a0a2e5c3ae6d14c2e45b9` (2026-11-16).
**Nhánh:** `codex/audit-21-remediation`.
**Hướng dẫn:** [ANTIGRAVITY_IMPLEMENTATION_GUIDE.md](ANTIGRAVITY_IMPLEMENTATION_GUIDE.md) · **Bàn giao:** [ANTIGRAVITY_HANDOVER.md](ANTIGRAVITY_HANDOVER.md)
**Bằng chứng và tiêu chí đóng:** [CONSOLIDATED_AUDIT_AND_REMEDIATION_2026-11-16.md](CONSOLIDATED_AUDIT_AND_REMEDIATION_2026-11-16.md)

Quy ước trạng thái: **Đóng (code/fixture)** = root cause sửa ở boundary thật, có regression, đạt tiêu chí đóng ở mức code. **Một phần** = sửa đúng hướng nhưng còn thiếu loại test/đo lường mà tiêu chí đóng yêu cầu. Không mục nào được coi là “nghiệm thu đầy đủ” khi native/official còn pending. Khi hai auditor khác nhau, tracker lấy kết luận chặt hơn.

| ID | Mức | Phase | Trạng thái | Commit | Kiểm chứng | Native / official | Phần còn lại |
|---|---|---|---|---|---|---|---|
| F01 | P1 | P1 | Đóng (code/fixture) | `5bb5155` | PHASE1_VERIFICATION; cả hai auditor CLOSED | Chưa test micro thật | Native mic/ARM64. |
| F02 | P1 | P1 | Một phần | `b25d962`, R1 | Gate hidden Vault; R1 thêm gate cho gửi sticker | Chưa nghiệm thu PIN TUI | Thiếu fixture account-switch/restart (test hiện ở mức filter). |
| F03 | P1 | P1 | Một phần | `b25d962`, R1 | Tombstone sau ACK; R1 chặn logout khi còn gửi/xóa đang chờ | — | Thiếu test qua `StoreCommand::Delete`→`DeleteDone` và fault injection DB sau ACK. |
| F04 | P2 | P2 | Một phần | `99674a0` | RuntimeStamp chặn event cũ | — | Test mới ở mức stamp; cần test tại boundary event. |
| F05 | P2 | P2 | Một phần | `99674a0`, R1 | Outcome có kiểu; R1 sửa race hydrate làm mất tin lạc quan + test | — | Thiếu test reorder ACK/echo/restart. |
| F06 | P2 | P2 | Một phần | `99674a0`, R1 | Writer actor có giới hạn, drain khi thoát (3 test); R1 sửa thứ tự snapshot | — | Chưa đo p95/p99 responsiveness. |
| F07 | P2 | P2 | Đóng (code/fixture) | `99674a0` | Migration 13 trong transaction, test fault/repair | — | — |
| F08 | P2 | P2 | Một phần | `99674a0`, R1 | Intent add/remove; R1 chặn double-click gửi “add” lặp | — | Thiếu test dispatch/request capture và stale completion. |
| F09 | P2 | P2 | Một phần | `99674a0`, R1 | Thread thiết bị, Arc snapshot, giới hạn 5 phút/64 MiB; R1: Start bị bỏ qua luôn trả Failed | Chưa test micro thật | Thiếu benchmark trước/sau. |
| F10 | P2 | P2 | Đóng (code/fixture) | `99674a0`, R1 | R1 thay nhãn “Mã hóa đầu cuối: Tiêu chuẩn” bằng tóm tắt theo trạng thái thật + test ma trận | — | — |
| F11 | P2 | P2 | Đóng (code/fixture) | `99674a0`, R1 | Deadline + cancellation mọi lần ghi; R1 propagate lỗi first ping / GET_MSG | Chưa soak mạng | — |
| F12 | P2 | P2 | Đóng (code/fixture) | `99674a0` | Redaction QR/auth với secret giả | — | — |
| F13 | P2 | P2 | Đóng (local, thử nghiệm) | `99674a0` | ADR-0038; test substitution không ghi session/trust | Zalo: Unknown — AMBIGUOUS | Contract Zalo chưa có bằng chứng E1/E2. |
| F14 | P3 | P3 | Đóng (code/fixture) | `99674a0` | Mở thư mục ngoài UI thread, 3 test outcome | Chưa kiểm Unicode/shell trên ARM64 | — |
| F15 | P3 | P3 | Đóng (code/fixture) | `99674a0` | OS CSPRNG, rejection sampling, 4 test | Chưa login live | — |
| F16 | P3 | P3 | Đóng (code/fixture) | `99674a0` | `endpoints::STICKER_CATALOG_FALLBACK`, 2 test | — | — |
| F17 | P3 | P3 | Đóng (code/fixture) | `99674a0` | CHECKED/FAILED/SKIPPED/HEURISTIC; `scripts/tests/test_audit_prepush.py` 4 test | — | — |
| F18 | P2 | P2 | Đóng (code/fixture) | `99674a0`, R1 | GUI/poll dùng account UID; R1 áp dụng cho refresh và group history của TUI | — | Phone sync tổng quát: AMBIGUOUS. |
| F19 | P2 | P2 | Đóng (local, thử nghiệm) | `99674a0` | Khớp vector Signal công khai; schema 9 reset verification | Zalo parity: Unknown | — |
| F20 | P2 | P2 | Một phần | `99674a0` | Một popover tài khoản duy nhất | Chưa kiểm DPI/viewport | Thiếu test command/state. |
| F21 | P2 | P2 | Một phần | `99674a0`, R1 | Desktop HWND, lease tới khi native hoàn tất; R1 hiện lý do lỗi | Cần Windows ARM64 thật | Màn khóa chưa có đường đăng xuất khi WinRT treo vô hạn. |

## Cổng phase và vòng audit

| Cổng | Trạng thái | Bằng chứng |
|---|---|---|
| P1 triển khai + kiểm chứng | Xong | `5bb5155`, `b25d962`, [PHASE1_VERIFICATION.md](PHASE1_VERIFICATION.md) |
| P2 + P3 triển khai + kiểm chứng | Xong | `99674a0`; gate bên dưới |
| Pro auditor 1 | CHANGES REQUIRED (3 major, 9 minor) | [AUDITOR_REVIEWS.md](AUDITOR_REVIEWS.md) |
| Pro auditor 2 | APPROVE (5 minor) | [AUDITOR_REVIEWS.md](AUDITOR_REVIEWS.md) |
| Vòng sửa R1 | Xong | xem AUDITOR_REVIEWS.md |
| Re-review của cả hai | Cả hai APPROVE; vòng R2 xử lý 2 minor | [AUDITOR_REVIEWS.md](AUDITOR_REVIEWS.md) |
| Merge local/GitHub `main` | Xong (fast-forward, maintainer xác nhận 2026-11-16) | `main` = `codex/audit-21-remediation` |

## Gate đã chạy (Windows x64, rustc 1.99.1, `CTDOZ_DATA_DIR` tạm, `CTDOZ_ALLOW_TEST_LOOPBACK=1`)

| Lệnh | Sau `99674a0` | Sau R1 và R2 (giống nhau) |
|---|---|---|
| `cargo fmt --all -- --check` | exit 0 | exit 0 |
| `cargo check --locked --all-targets` | exit 0 | exit 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 | exit 0 |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | exit 0 | exit 0 |
| `cargo test --locked --no-fail-fast` | 907 passed, 0 failed, 13 ignored | 909 passed, 0 failed, 13 ignored |
| `cargo test --locked --no-fail-fast --features experimental-e2ee` | 910 passed, 0 failed, 13 ignored | 912 passed, 0 failed, 13 ignored |
| `python -I scripts/tests/test_audit_prepush.py` | 4 OK | 4 OK |

Test ignored không được tính là pass. Log nằm trong `implementation-logs/` (gitignore). `tests/audit_regressions.rs` cần `CTDOZ_ALLOW_TEST_LOOPBACK=1` vì cfg(test) không áp dụng cho integration test.
