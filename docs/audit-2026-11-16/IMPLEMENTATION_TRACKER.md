# Tracker triển khai — Các phát hiện F01–F21

**Trạng thái:** Phase 1 đã qua gate code/fixture, đang triển khai Phase 2. Chưa bắt đầu P3; chưa audit cuối bởi hai Pro, chưa push/merge main.  
**Mốc audit lịch sử:** `main` tại `f0d3206e875d083c7f8a0a2e5c3ae6d14c2e45b9` (2026-11-16); kiểm tra lại checkout trước khi làm.  
**Hướng dẫn:** [ANTIGRAVITY_IMPLEMENTATION_GUIDE.md](ANTIGRAVITY_IMPLEMENTATION_GUIDE.md)  
**Bằng chứng và tiêu chí đóng finding:** [CONSOLIDATED_AUDIT_AND_REMEDIATION_2026-11-16.md](CONSOLIDATED_AUDIT_AND_REMEDIATION_2026-11-16.md)

## Thứ tự ba phase đã được cho phép

1. P1: F01–F03.
2. P2: F04–F13, F18–F21.
3. P3: F14–F17.

Nhóm A–D trong báo cáo audit là kế hoạch cũ và không thay cho thứ tự này. Trạng thái bên dưới được cập nhật theo từng gate thực tế; báo cáo audit gốc giữ nguyên để truy vết baseline. Chỉ cập nhật hàng theo bằng chứng source/check/native/auditor thực tế; phân biệt kiểm chứng code/fixture với nghiệm thu native hoặc official.

| ID | Mức độ | Phase được phép | Trạng thái ban đầu | Tham chiếu triển khai | Tham chiếu check/bằng chứng | Nghiệm thu native / official | Ghi chú / phần còn lại |
|---|---|---|---|---|---|---|---|
| F01 | P1 | P1 | Qua gate code/fixture P1 | `5bb5155` | [PHASE1_VERIFICATION.md](PHASE1_VERIFICATION.md) | Chưa nghiệm thu native | Chờ audit cuối sau 3 phase. |
| F02 | P1 | P1 | Qua gate code/fixture P1 | `b25d962` | [PHASE1_VERIFICATION.md](PHASE1_VERIFICATION.md) | Chưa nghiệm thu native | Chờ audit cuối sau 3 phase. |
| F03 | P1 | P1 | Qua gate code/fixture P1 | `b25d962` | [PHASE1_VERIFICATION.md](PHASE1_VERIFICATION.md) | Chưa nghiệm thu native | Chờ audit cuối sau 3 phase. |
| F04 | P2 | P2 | Đang triển khai | PENDING | PENDING | PENDING | |
| F05 | P2 | P2 | Đang triển khai | PENDING | PENDING | PENDING | |
| F06 | P2 | P2 | Đang triển khai | PENDING | PENDING | PENDING | |
| F07 | P2 | P2 | Đang triển khai | PENDING | PENDING | PENDING | |
| F08 | P2 | P2 | Đang triển khai | PENDING | PENDING | PENDING | |
| F09 | P2 | P2 | Đang triển khai | PENDING | PENDING | PENDING | Phụ thuộc ranh giới ownership F01. |
| F10 | P2 | P2 | Đang triển khai | PENDING | PENDING | PENDING | Phạm vi assurance E2EE thử nghiệm. |
| F11 | P2 | P2 | Đang triển khai | PENDING | PENDING | PENDING | Cần bằng chứng trước khi đổi protocol behavior. |
| F12 | P2 | P2 | Đang triển khai | PENDING | PENDING | PENDING | Chỉ dùng secret giả lập. |
| F13 | P2 | P2 | Đang triển khai | PENDING | PENDING | PENDING | Thử nghiệm; contract có thể vẫn là `Unknown — AMBIGUOUS`. |
| F14 | P3 | P3 | Chưa bắt đầu | PENDING | PENDING | PENDING | |
| F15 | P3 | P3 | Chưa bắt đầu | PENDING | PENDING | PENDING | Giữ tương thích login request. |
| F16 | P3 | P3 | Chưa bắt đầu | PENDING | PENDING | PENDING | Giữ endpoint/service-map behavior. |
| F17 | P3 | P3 | Chưa bắt đầu | PENDING | PENDING | PENDING | Không hàm ý có hành động tag/release. |
| F18 | P2 | P2 | Đang triển khai | PENDING | PENDING | PENDING | Giữ khác biệt adapter; identity semantics phải dựa trên bằng chứng. |
| F19 | P2 | P2 | Đang triển khai | PENDING | PENDING | PENDING | Thử nghiệm; không giả định đã có vector/parity Zalo. |
| F20 | P2 | P2 | Đang triển khai | PENDING | PENDING | PENDING | Kiểm viewport/DPI native có thể còn pending. |
| F21 | P2 | P2 | Đang triển khai | PENDING | PENDING | PENDING | Cần desktop HWND và nghiệm thu Windows ARM64 native. |

## Cổng phase và vòng audit

| Cổng | Trạng thái | Bằng chứng / tham chiếu |
|---|---|---|
| Đã triển khai và kiểm chứng riêng từng finding P1 | Chưa bắt đầu | PENDING |
| Đã review handoff P1; ghi residual trước P2 | Chưa bắt đầu | PENDING |
| Đã triển khai và kiểm chứng riêng từng finding P2 | Chưa bắt đầu | PENDING |
| Đã review handoff P2; ghi residual trước P3 | Chưa bắt đầu | PENDING |
| Đã triển khai và kiểm chứng riêng từng finding P3 | Chưa bắt đầu | PENDING |
| Review của Pro auditor 1 sau tất cả phase | Chưa bắt đầu | PENDING |
| Review của Pro auditor 2 sau tất cả phase | Chưa bắt đầu | PENDING |
| Đã xử lý repair iteration và nhận re-review của cả hai | Chưa bắt đầu | PENDING |
| Bàn giao merge vào local/GitHub `main` | Chưa bắt đầu | PENDING |

Dùng mẫu handoff trong hướng dẫn triển khai. Ghi command chính xác, exit status, test ignored/failed, native target/device và tham chiếu auditor. Không sao chép kết quả audit lịch sử thành kết quả mới. Ô trống/PENDING không phải pass; không thể chuyển phase chỉ bằng cách sửa chữ trong tracker.
