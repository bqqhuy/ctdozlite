# Chấp thuận đồng thuận — data_logic

Ngày 16/11/2026, baseline `f0d3206e875d083c7f8a0a2e5c3ae6d14c2e45b9`.

Đã đọc toàn bộ `AUDIT_REPORT.md`, gồm đủ F01–F17, mức độ, số lượng, tối ưu, code thừa, nghi vấn bị bác bỏ và giới hạn. Đồng ý kết luận **17 findings: 3 P1 / 10 P2 / 4 P3**.

## Quyết định từng finding

| ID | Mức | Quyết định |
|---|---|---|
| F01 | P1 | ACCEPT: voice owner/thread lifecycle và thao tác gửi sau chuyển profile. |
| F02 | P1 | ACCEPT: TUI bypass display gate; chỉnh nguồn ADR thành `docs/adr/0034-hidden-chat-pin-compatibility.md`, hydrate chính xác `src/tui.rs:201-204`. |
| F03 | P1 | ACCEPT: upsert snapshot không xóa DB/tạo tombstone; hydrate hồi sinh. |
| F04 | P2 | ACCEPT: event xếp hàng cũ hồi sinh UI; không khẳng định cache sai owner. |
| F05 | P2 | ACCEPT: dropped ACK và lỗi không có client ID; rejected khác unknown. |
| F06 | P2 | ACCEPT: synchronous persistence/cache trên thread TUI. |
| F07 | P2 | ACCEPT: failure-path migration13; không nói mọi upgrade đều hỏng. |
| F08 | P2 | ACCEPT: local remove nhưng backend add payload. |
| F09 | P2 | ACCEPT: clone samples mỗi repaint, audio setup/encode UI; chưa benchmark FPS. |
| F10 | P2 | ACCEPT: E2EE assurance không dựa capability/session thật. |
| F11 | P2 | ACCEPT: selected ws.send await thiếu timeout làm mất liveness khi write Pending. |
| F12 | P2 | ACCEPT: raw verbose response chưa redact; tác động tùy dữ liệu đầu response. |
| F13 | P2 | ACCEPT: experimental identity binding validation; không chứng minh MITM/confidentiality exploit live. |
| F14 | P3 | ACCEPT: diagnostics I/O/shell trên UI và nuốt lỗi. |
| F15 | P3 | ACCEPT: LCG salt; không suy diễn phá password. |
| F16 | P3 | ACCEPT: fallback runtime sticker host ngoài endpoints, xác nhận `src/chat.rs:3296`. |
| F17 | P3 | ACCEPT: release assurance script không thực hiện 4-point check, warnings vẫn success; compilation gate vẫn hoạt động. |

Đã gửi root yêu cầu sửa path ADR F02; đây là chỉnh citation, không thay đổi finding/count/severity. Chấp thuận cuối có hiệu lực sau khi citation này được sửa. Các placeholder kết quả Cargo chỉ được root thay bằng log thực; agent này không xác nhận test chưa kết thúc.

## Tối ưu và dọn code

ACCEPT đề xuất O(S) voice clone, GUI visible-index scan, TUI O(T×M) merge, delta/single-writer persistence, bounded recording. Không hứa speedup nếu chưa đo. Đã đọc thêm `workspace.rs:2593-2618` và wrapper `:5052`: wrapper method không có caller nội bộ qua rg; free function có caller thật `:2720/:2723`, phải giữ. Chỉ đề nghị xem xét xóa wrapper/suppression sau cfg/API check, không cho phép xóa rộng.

## Bằng chứng và giới hạn

D1/D5 có hai SQLite in-memory control-flow models chạy thành công trong `reproduce_data_models.py`, output `data-model-results.json`. Không phải thực thi Rust MessageStore, không GUI/native test và không production incident. Không sử dụng API Zalo thật, session/DB/capture người dùng. Báo cáo theo rủi ro không chứng nhận mọi dòng source, ARM64 hoặc interoperability.

Không thay đổi source/test hiện có; chỉ tạo báo cáo và artifact audit.
