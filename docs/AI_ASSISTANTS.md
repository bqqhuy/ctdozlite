# Hướng dẫn dùng chung cho các AI coding agent

Repo áp dụng bốn nguyên tắc từ
[multica-ai/andrej-karpathy-skills](https://github.com/multica-ai/andrej-karpathy-skills):
nghĩ trước khi sửa, ưu tiên đơn giản, thay đổi đúng phạm vi và làm đến khi kiểm chứng
được mục tiêu. Bản đối chiếu: `2c606141936f1eeef17fa3043a72095b4765b9c2`.
Đây là bộ hướng dẫn cộng đồng lấy cảm hứng từ Karpathy, không phải plugin chính thức
của ông hoặc một cơ chế bảo đảm agent luôn làm đúng.

## Cách áp dụng

Nguồn quy tắc duy nhất là [AGENTS.md](../AGENTS.md), mục **Karpathy-inspired coding
discipline**. Đã thích nghi các nguyên tắc với repo và cách làm việc đã thống nhất:
agent tiếp tục công việc được phép, tự xử lý lựa chọn thường lệ, chỉ hỏi khi thiếu
thông tin ảnh hưởng đáng kể đến tính đúng đắn/phạm vi/quyết định khó đảo ngược.
Không thay quy tắc bảo mật, bằng chứng giao thức, Spec Kit hoặc điều kiện phát hành.
Không yêu cầu test mới cho mỗi sửa tài liệu hay thay đổi nhỏ dễ đảo ngược.

| Công cụ | Điểm vào trong repo | Cách dùng |
| :--- | :--- | :--- |
| Codex | `AGENTS.md`; `.agents/skills/karpathy-guidelines/SKILL.md` | AGENTS là hướng dẫn dự án; có thể gọi `$karpathy-guidelines` khi skill đã được công cụ nhận diện |
| Claude Code | `CLAUDE.md`; `.claude/skills/karpathy-guidelines/SKILL.md` | CLAUDE yêu cầu đọc AGENTS; skill có thể gọi bằng `/karpathy-guidelines` |
| Gemini CLI | `GEMINI.md` | Import `@./AGENTS.md`; dùng `/memory show` để kiểm tra ngữ cảnh, `/memory refresh` để tải lại khi cần |
| Cursor | `.cursor/rules/karpathy-guidelines.mdc` | Rule `alwaysApply: true` yêu cầu đọc AGENTS; mở repo và kiểm tra Project Rules |
| GitHub Copilot | `.github/copilot-instructions.md` | Hướng dẫn workspace yêu cầu đọc AGENTS; hỗ trợ cụ thể tùy sản phẩm/chế độ |
| Công cụ khác | `AGENTS.md` | Nếu không tự nhận AGENTS, chỉ định đọc file trước khi làm việc |

Skill không phải điều kiện bắt buộc: các nguyên tắc đã nằm trong hướng dẫn dự án.
Không cài marketplace plugin hay sửa `~/.claude`, `~/.codex`, `~/.gemini` hoặc thiết
lập tài khoản toàn máy. Các adapter là file thường để checkout Windows không phụ
thuộc symlink. Không cần API key, thư viện runtime hoặc thay bản cài ARM64.
Phiên đang mở có thể cần tải lại project/context để nhận skill/rule mới.

## Quan hệ với Spec Kit và Graphify

- Karpathy-inspired: cách thực hiện và rà soát thay đổi.
- Spec Kit: yêu cầu, kế hoạch, task và kiểm chứng; không tạo bản trạng thái riêng cho từng AI.
- Graphify: tra cứu roadmap/mã/test để chọn đúng nguồn đọc; không tự đóng task.

Cả ba cùng dùng roadmap đã chốt v0.008–v0.011. Việc thêm hướng dẫn không mở rộng
phạm vi ứng dụng, đổi privacy, cấp phép commit/push hoặc tạo release.

## Kiểm tra khi dùng công cụ thật

Yêu cầu agent nêu các file hướng dẫn đã nạp và phân biệt code/test/native acceptance.
Kiểm tra diff có đúng phạm vi, không hỏi lại quyền đã có, không tự refactor lân cận,
và báo đúng các check đã chạy. Nếu agent không thấy skill/rule, tải lại ngữ cảnh hoặc
chỉ định đường dẫn AGENTS rõ ràng; không cài plugin thứ hai với một bộ quy tắc khác.

Đã kiểm tra cấu trúc adapter, tham chiếu file và diff trong workspace. Chưa chạy từng
CLI/IDE trên máy người dùng; mức tuân thủ cần kiểm tra trong công cụ thực tế.

## Cập nhật

Sửa nguyên tắc tại AGENTS.md, giữ các adapter ngắn và trỏ về đó. Khi cập nhật từ
upstream, đọc diff ở commit cụ thể rồi thích nghi; không `curl` ghi đè CLAUDE.md hoặc
AGENTS.md. Repo upstream khai báo MIT trong README/metadata skill; phần tích hợp
này diễn giải các nguyên tắc và ghi nguồn, không vendoring plugin hay sao chép toàn
bộ ví dụ/quy tắc upstream.
