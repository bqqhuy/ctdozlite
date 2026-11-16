# Bàn giao cho Antigravity — Khắc phục audit F01–F21

Tài liệu này dành cho agent tiếp quản (Antigravity) để tiếp tục triển khai, kiểm chứng hoặc audit. Đọc kèm:
[ANTIGRAVITY_IMPLEMENTATION_GUIDE.md](ANTIGRAVITY_IMPLEMENTATION_GUIDE.md) (ranh giới và kỷ luật), [ANTIGRAVITY_REVIEW.md](ANTIGRAVITY_REVIEW.md) (những điểm Antigravity từng hiểu sai), [IMPLEMENTATION_TRACKER.md](IMPLEMENTATION_TRACKER.md) (trạng thái từng finding), [AUDITOR_REVIEWS.md](AUDITOR_REVIEWS.md) (hai Pro auditor và vòng sửa), `re-notes/43-audit21-remediation.md` và `docs/adr/0038-identity-bound-trial-verification.md`.

## Mẫu handoff

```text
Branch / commit baseline: codex/audit-21-remediation, từ main f0d3206
Phạm vi working tree đã kiểm tra: sạch sau commit cuối của nhánh
Phase hoàn thành: P1, P2, P3 (code/fixture); vòng sửa R1
Finding ID đã thay đổi: F01–F21
Finding ID còn một phần: F02, F03, F04, F05, F06, F08, F09, F20, F21 (xem tracker)
Implementation commit(s): 5bb5155, b25d962 (P1); 99674a0 (P2+P3); a41b49c (R1); commit R2 ngay sau đó
Test/command đã chạy: xem bảng gate trong IMPLEMENTATION_TRACKER.md
Test ignored/failed: 13 ignored (cần asset/thiết bị), 0 failed
Bằng chứng native Windows ARM64: PENDING — chưa chạy trên thiết bị
Bằng chứng Zalo parity: F13/F19/F18 phone sync = Unknown — AMBIGUOUS; không có E1/E2 mới
Auditor 1: vòng 1 CHANGES REQUIRED (3 major) → re-review R1: APPROVE
Auditor 2: vòng 1 APPROVE (5 minor) → re-review R1: APPROVE
Merge target: main (local + GitHub) sau khi cả hai auditor approve
```

## Cách chạy gate đúng

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:CMAKE_POLICY_VERSION_MINIMUM = "3.5"
$env:CTDOZ_ALLOW_TEST_LOOPBACK = "1"        # bắt buộc cho tests/audit_regressions.rs
$env:CTDOZ_DATA_DIR = "<thư mục tạm mới>"   # không bao giờ dùng dữ liệu thật
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --all-targets --locked -- -D warnings
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked --no-fail-fast
cargo test --locked --no-fail-fast --features experimental-e2ee
python -I scripts/tests/test_audit_prepush.py
powershell scripts/audit-prepush.ps1        # in CHECKED/FAILED/SKIPPED/HEURISTIC
```

Khi đếm test, cộng mọi dòng `test result:` của từng binary; không lấy một dòng. Ignored không tính là pass.

## Những điều phải hiểu đúng khi làm tiếp

1. **TUI history writer** (`src/tui.rs`, `StoreWriter`): mọi ghi SQLite/cache đi qua một actor với 16 slot. Lệnh xóa giữ trước một slot từ trước khi gọi server, để tombstone luôn ghi được sau ACK. Khi thoát, `shutdown_writer` drain có giới hạn 5 giây và chỉ báo “đã lưu” khi worker xác nhận. Không thêm đường ghi DB đồng bộ trên luồng UI.
2. **Hydrate là bất đồng bộ.** `BgEvent::Hydrated` phải hợp nhất (`merge_hydrated`), không được thay danh sách thread, vì người dùng có thể đã gửi tin hoặc poll đã về trước đó.
3. **Runtime stamp.** Mọi event nền và lệnh ghi mang `RuntimeStamp` (owner + runtime id). Event của phiên cũ phải bị bỏ trước khi đổi state. Không được bỏ qua kiểm tra này để “tiện”.
4. **E2EE là thử nghiệm.** Không UI nào được hiển thị nội dung kiểu “đã mã hóa đầu cuối/tiêu chuẩn”. Nhóm chưa hỗ trợ. Fingerprint theo scheme Signal (5200 vòng, vector công khai), nhưng tương thích Zalo vẫn là Unknown — AMBIGUOUS. XEdDSA phải dùng đúng DH identity; không thêm lại fallback Ed25519 độc lập.
5. **Protocol.** Không đổi command ID, payload, endpoint hay heartbeat khi chưa có bằng chứng E1/E2. Thay đổi socket F11 chỉ là deadline/cancellation cục bộ. Host fallback sticker nằm ở `endpoints::STICKER_CATALOG_FALLBACK`; server `dataUrl` luôn ưu tiên.
6. **Phím TUI:** `y` = trả lời, `Y` = gửi lại tin lỗi/chưa rõ với cùng client ID.
7. **Prepush (`scripts/audit-prepush.ps1`):** stub scan chỉ là HEURISTIC, không chứng nhận UI. Khi build có tag, mọi lệch giữa tag, `PRODUCT_VERSION`, test updater, Cargo.toml/Cargo.lock và CHANGELOG đều làm script fail. Script không tạo tag hay push.

## Việc còn lại theo ưu tiên

| Ưu tiên | Việc | Finding |
|---|---|---|
| 1 | Nghiệm thu native Windows ARM64: micro/ghi âm, Windows Hello (HWND, timeout, hủy), mở thư mục Unicode, DPI popover | F01, F09, F14, F20, F21 |
| 2 | Màn khóa: đường đăng xuất khôi phục khi WinRT không bao giờ hoàn tất; xử lý xác thực dài hơn 30 giây | F21 |
| 3 | Test tại boundary event: stale event sau logout, `StoreCommand::Delete`→`DeleteDone`, fault injection DB sau ACK | F03, F04 |
| 4 | Test race: reorder ACK/echo/restart cho TUI; dispatch reaction và stale completion | F05, F08 |
| 5 | Fixture account-switch/restart cho hidden chat TUI; test command/state cho popover tài khoản | F02, F20 |
| 6 | Đo lường: p95/p99 responsiveness của TUI; benchmark voice trước/sau | F06, F09 |
| 7 | Khóa reaction pending: thêm timeout/revision để nhả khóa nếu reply của worker bị mất | F08 |
| 8 | Bằng chứng Zalo (E1 capture) cho E2EE và phone sync trước khi tuyên bố parity | F13, F18, F19 |

## Quy tắc khi audit lại

- Đọc lại source trên commit hiện tại, vì số dòng trong báo cáo cũ thuộc snapshot `f0d3206`.
- Mỗi kết luận phải có file:line và kịch bản lỗi cụ thể. Không chép kết quả test lịch sử thành kết quả mới.
- Để riêng “code/fixture verified” với “native/official verified”.
