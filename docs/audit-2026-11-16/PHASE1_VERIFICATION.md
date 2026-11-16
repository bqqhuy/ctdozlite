# Kiểm chứng Phase 1 — P1

Baseline: `f0d3206`; nhánh `codex/audit-21-remediation`.

| Mục | Commit | Kết quả ở mức code/fixture |
|---|---|---|
| F01 | `5bb515565247a06dc1d2729c843c30b544a621a5` | Draft/command/event ghi âm gắn owner + runtime + thread; hủy khi đổi context; WAV tạo tên riêng an toàn. |
| F02 | `b25d9624df1b0cf495f9e8f2ad5b1bf68cb4dde0` | TUI dùng chung hidden Vault, restore/read/unlock/relock, gate list/search/view/action; không lộ nội dung khi membership unknown. |
| F03 | `b25d9624df1b0cf495f9e8f2ad5b1bf68cb4dde0` | Sau server ACK, commit tombstone trên background trước local success; batch replay được DB filter; identity filter không xóa nhầm sender/server ID khác. |

Gates đã chạy trên Windows x64, rustc 1.99.1, data root temp riêng:

- `cargo fmt --all`: exit 0.
- `cargo check --all-targets --locked`: exit 0, bản cuối không có compiler warning.
- `cargo test --locked --lib`: 786 passed, 0 failed, 10 ignored.
- `cargo clippy --all-targets --locked -- -D warnings`: exit 0 sau sửa một lint clone trong test.
- `git diff --check`: pass.

Log local nằm trong `implementation-logs/phase1-*` và được gitignore theo quy tắc log.
Lần check đầu bắt các handler/test đang nối dở; bản cuối đã qua. Không dùng kết quả
audit baseline 856/859 để thay cho các lệnh mới này. Check/test sau toàn bộ ba phase
và hai Pro reviewer vẫn chưa thực hiện ở mốc này.

Chưa nghiệm thu micro, PIN/TUI, native ARM64 hoặc live Zalo. Đây là gate code/fixture
để bắt đầu P2, không phải chứng nhận thiết bị hoặc claim toàn bộ 21 mục đã xong.
