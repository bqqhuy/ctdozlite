# Sidebar: sửa bố cục banner đồng bộ và hàng tìm kiếm

Ngày: 24/11/2026 (Asia/Saigon). Phạm vi: ảnh người dùng cung cấp trong chat.

## Hiện tượng và nguyên nhân

- USER: CTA đồng bộ thành dải mỏng, tiêu đề nằm giữa khoảng trống và nút tạo
  nhóm sát mép phải bị cắt. Ảnh sáng; chưa có thông tin DPI chính xác của ảnh.
- CODE: CTA dùng `Button::small()` với chiều cao yêu cầu 0; tiêu đề dùng
  `add_sized` mà chưa chỉ định căn trái; nút đóng có khung riêng làm hàng trên
  nặng hơn hành động chính.
- CODE: search rỗng chỉ trừ chiều rộng biểu tượng tìm kiếm, bỏ quên biểu tượng
  archive và khoảng cách đi kèm. Chiều rộng thực vượt ngân sách hàng và đẩy
  nút tạo nhóm ra ngoài mép panel. Trạng thái có query đã chừa trailing slot.

## Thiết kế và phạm vi sửa

Giữ banner ở dưới bộ lọc, trên danh sách hội thoại; giữ theme tokens, màu trạng
thái, nội dung remote config và luồng action/fencing hiện có. Tiêu đề căn trái
cạnh icon, nút đóng 24pt không khung; CTA ở hàng riêng, cao tối thiểu 32pt,
chữ trắng trên màu primary và wrap khi nội dung dài. Icon gợi ý và đóng dùng
Lucide hiện có. Search rỗng/có query đều dành trailing slot; PIN vẫn có slot riêng.
Giữ mô tả phụ và hành vi retry mà Antigravity vừa tích hợp; mô tả được wrap
tường minh. Sidebar thu gọn dưới 140pt dùng icon hành động/đóng và tooltip;
đang chạy hiển thị spinner thay vì dấu check thành công. Không wrap CTA thành
cột chữ trong chế độ sidebar chỉ có avatar/icon.

Hàng bộ lọc đo chữ với font render thật. Ngân sách nút phân loại được lấy sau
khi cấp vùng cho menu `⋯`, bao gồm spacing thực; đo cả nhãn đầy đủ và nhãn
`Thẻ ▾`, rồi dùng chevron khi không đủ chỗ. Khoảng cách 4/6/10pt theo chiều
rộng hàng. Giữ tooltip và cùng action mở labels; đây là fallback của fork.

Parity with Zalo PC/Web (source: `re-notes/73-sync-origin-timeouts-and-startup-completion.md`,
E2) chỉ ở intent/gate của banner. Các số đo trên là sửa trình bày của fork theo
ảnh người dùng, không khẳng định là kích thước native. Không đổi giao thức hoặc
điều kiện cho phép đồng bộ. Không thiết kế lại toàn sidebar.

## Kiểm chứng sau khi tiếp tục

- `cargo test --locked --lib gui::sync_banner::tests::`: 8/8 đạt, gồm CTA đủ
  cao và chứa chữ, title căn trái, nội dung dài/các trạng thái không nở panel.
- `cargo test --locked --lib long_sync_banner_and_search_never_expand_sidebar_content`:
  đạt. Thêm seed panel 190/240/320pt; scale fixture 1/1.25/1.5/2. Đo cả vùng
  bấm và chữ của bộ lọc; hàng search/add/group nằm trong mép panel trước,
  trong và sau search/sync. App fixture cài font thật qua `from_context`.
- `cargo check --locked`: đạt trên nền mới trước sửa.
- `cargo build --locked --bin ctdozlite`: đạt sau sửa cuối.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked -- -D warnings`,
  `git diff --check`: đạt. Không chạy toàn bộ Cargo test suite trong lượt này.
- Thêm smoke fixture `--ui-smoke-sync-suggest` để render đúng banner gợi ý bằng
  dữ liệu giả, không worker/network/tài khoản thật.
- Đồng bộ thật với điện thoại và xác nhận của người dùng: chưa thực hiện.

## Ảnh render cuối

Native Windows ARM64, renderer DX12 trên Qualcomm Adreno 8cx Gen 3. Bốn
process smoke đều exit 0, mỗi process lưu 3 frame; đã xem frame thứ ba:

- `target/ui-review-sidebar-resumed-20261124/light-normal-03.png`: tab có khoảng
  cách, nhãn phân loại gọn `Thẻ ▾`, nút tìm kiếm/add/group không cắt mép; banner
  có title, mô tả wrap và CTA đủ cao.
- `target/ui-review-sidebar-resumed-20261124/light-wide-03.png`: nhãn đầy đủ
  `Phân loại ▾`, banner và hàng tìm kiếm nằm trong panel.
- `target/ui-review-sidebar-resumed-20261124/dark-normal-03.png`: nội dung CTA
  trắng đọc được trên nền primary, mô tả và title đọc được trên nền tối.
- `target/ui-review-sidebar-resumed-20261124/dark-compact-03.png`: banner icon
  gọn, không xếp CTA thành cột, không đẩy danh sách xuống một vùng quá cao.

Tham số smoke scale lần lượt 1 / 1.25 / 1 / 2; không thay Windows Display Scale.
Đây là kiểm tra render native với dữ liệu giả, chưa thay kiểm tra pointer,
keyboard và đồng bộ điện thoại thật. Ảnh rộng/compact còn ghi nhận header hội
thoại có chữ/icon đè nhau ở vùng chat bên phải; ngoài phạm vi sidebar trong
lượt này, không khẳng định toàn bộ workspace đã được sửa.

## Kết quả trung gian và chỉnh sửa đồng thời

- Bản trước chỉnh sửa đồng thời: 8/8 test `gui::sync_banner::tests::` và
  `long_sync_banner_and_search_never_expand_sidebar_content` đạt; debug binary
  build thành công. `cargo fmt --all -- --check` và `git diff --check` đạt.
- Native smoke trên Windows ARM/DX12, dữ liệu giả: light với tham số scale
  1 / 1.25 / 1.5, dark với 1 / 2. Đây là tham số smoke của egui, không phải
  bằng chứng đã đổi Windows Display Scale sang các phần trăm tương ứng.
- Ảnh trong `target/ui-review-sidebar-20261124/`: `light-100.png`,
  `light-125.png`, `light-150.png`, `dark-repeat-03.png`, `dark-200.png`.
  Dark repeat kiểm tra lại qua 3 frame chụp; CTA có chữ đầy đủ. Dark compact
  dùng icon hành động và đóng, không còn cột chữ cao.
- Ảnh normal-width còn phát hiện hai nhãn bộ lọc chạm nhau. Bổ sung đo chữ
  thực cho hai tab; `Phân loại ▾` → `Thẻ ▾` → chevron theo ngân sách còn lại,
  giữ tooltip và cùng action mở labels. Đây là fallback trình bày của fork.
- Đã tạm dừng khi có chỉnh sửa đồng thời. Người dùng báo Antigravity làm xong
  rồi cho tiếp tục; đã đọc lại nền mới, lỗi dấu ngoặc không còn, giữ các thay
  đổi độc lập về hidden-chat, worker, modal và theme. Kết quả trung gian ở trên
  chỉ là lịch sử; nghiệm thu cuối dùng mã và ảnh của lượt tiếp tục.
