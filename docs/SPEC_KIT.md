# Spec Kit trong fork

Áp dụng thử từ 2026-11-05 cho phần còn lại của v0.007-arm. Đây là công cụ phát triển,
không phải dependency Rust hay thành phần của bản cài ứng dụng.

## Cấu hình đã chọn

- GitHub Spec Kit **v1.3.0**, commit `34a0d388eac671d656eb05b125ad3ed6056168ed`.
- Preset chính thức `lean`; lệnh/template điều chỉnh qua cơ chế project-local override.
  Preset nội bộ `ctdo-lite` đăng ký đủ chín lệnh để CLI render override cả các skill phụ.
  Không lấy preset này từ catalog bên ngoài.
- Codex: skill trong `.agents/skills/`, gọi bằng `$speckit-…` trong chat.
- Claude hoặc agent khác: đọc cùng AGENTS và các tệp đặc tả; chưa cài thêm integration
  hay sao chép một bộ slash command riêng cho Claude.
- Không cài extension cộng đồng, hook/event tự chạy, MCP server hay workflow tự động.
  Workflow mẫu upstream có trong scaffold nhưng không được pilot tự gọi.
- Không sửa Cargo, mã ứng dụng, CI/build/release để tích hợp công cụ này.

Phiên bản/nguồn ghi trong [.specify/toolchain.json](../.specify/toolchain.json).
Giấy phép upstream giữ tại [.specify/LICENSE-spec-kit.txt](../.specify/LICENSE-spec-kit.txt).

## Nguồn chuẩn và nơi ghi thông tin

| Nội dung | Nơi duy trì |
| :--- | :--- |
| Quy tắc kỹ thuật | AGENTS.md và docs/standards/ |
| Thuật ngữ, INV-N | GLOSSARY.md |
| Quyết định kiến trúc | docs/adr/ |
| Hợp đồng giao thức và điều chưa chắc | re-notes/ |
| Hành vi và nghiệm thu một tính năng | specs/<tính-năng>/spec.md |
| Cách triển khai, phụ thuộc | plan.md cùng thư mục |
| Task và trạng thái thực hiện | tasks.md cùng thư mục |
| Kết quả thật, phạm vi và giới hạn | verification.md cùng thư mục |
| Phạm vi bản phát hành | [v0.007-arm-plan.md](releases/v0.007-arm-plan.md) |

Constitution chỉ dẫn tới nguồn chuẩn, không tạo bộ quy định thứ hai. GitHub issue/PR
vẫn quản lý vòng đời ticket theo issue-tracker standard; task cục bộ không thay nhãn/status ticket.
Bản ghi phiên cũ là lịch sử; kết quả mới ghi vào verification.md, release plan chỉ liên kết.

## Dùng trong công việc

Tiếp tục tính năng hiện có bằng lệnh trong chat, ví dụ:

```text
$speckit-analyze specs/001-group-governance
$speckit-implement specs/001-group-governance
$speckit-converge specs/001-group-governance
```

Đường dẫn được lấy từ đối số, rồi checkout-local `.specify/feature.json`, cuối cùng
là `default_feature` trong `.specify/project.json`. File feature.json được ignore;
clone mới vẫn có default được quản lý bằng Git. Các đường dẫn phải nằm trong repo.
Agent khác có thể được yêu cầu đọc bốn tệp cùng thư mục và làm theo AGENTS.

Tính năng mới dùng specify → plan → tasks → implement → converge ở mức cần thiết.
Không chạy lại các pha chỉ để thay trạng thái, không ghi đè yêu cầu hoặc task ID cũ.
Lệnh clarify/checklist/analyze dùng khi có ích, không trở thành thủ tục bắt buộc.
Phạm vi người dùng đã đồng ý không cần xin lại giữa mỗi tài liệu. Việc cần bằng chứng
bổ sung được ghi blocker; công việc độc lập đã được ủy quyền vẫn tiếp tục.

Checkbox chỉ đóng task có tiêu chí đã đạt. Phân biệt rõ **chưa làm / một phần / đã có mã /
qua kiểm thử / xác nhận trên thiết bị**. Feature có task thiết bị chưa đạt chưa được
báo là hoàn tất toàn bộ. Ghi ignored/failed và giới hạn; không suy từ test sang iPhone thật.

## Cài công cụ hoặc tái tạo skill

Python 3.11+ và uv; công cụ nằm ngoài runtime ứng dụng:

```bash
uv tool install --from 'git+https://github.com/github/spec-kit.git@34a0d388eac671d656eb05b125ad3ed6056168ed' specify-cli
specify --version
specify integration status
```

Trong workspace này công cụ được cài từ checkout đã kiểm tra đúng commit trên.
Nguồn CLI được ghim; đây không phải lock đầy đủ mọi dependency Python của công cụ.
Không dùng `specify init --here --force` trên working tree đang có thay đổi.
Để tái tạo skill từ override đã review:

```bash
specify integration upgrade codex
```

Nếu CLI báo skill bị sửa, kiểm tra diff trước; thử nâng cấp trong checkout/thư mục tạm.
Không tự thêm --force hoặc nâng phiên bản từ main. Các override tại
`.specify/templates/overrides/` là nguồn điều chỉnh; `.agents/skills/` là bản render.
Nâng cấp Spec Kit là thay đổi tooling riêng, cần đọc diff scaffold và giữ cấu hình dự án.

Scaffold chính thức có script tạo nhánh; pilot dùng đường dẫn feature trực tiếp,
không gọi create-new-feature để tiếp tục các spec đã có trên branch hiện tại.
Spec Kit không tự ủy quyền commit, push, build phát hành, gửi tin hay mutation tài khoản thật.

## Kiểm chứng tích hợp ngày 2026-11-05

- `specify --version`: 1.3.0.
- `specify integration status`: OK; 0 modified/missing managed files, 0 invalid paths.
- `specify artifact list --json`: chín command có project override đang active;
  cả chín skill render chứa source override và quy tắc tiếp tục phần đã được ủy quyền.
- Kiểm tra JSON/default feature, hash manifest, 4 bộ spec/plan/tasks/verification,
  19 yêu cầu có task liên kết, liên kết Markdown và task thiết bị chưa bị đóng: đạt.
- `git diff --check`: đạt. Không chạy lại Cargo test vì đợt tích hợp chỉ đổi tooling/tài liệu.

Đợt tích hợp không xác nhận thêm hành vi v0.007; baseline và các thiếu sót vẫn giữ
nguyên trong từng verification.md. Chưa commit/push/build/release.
