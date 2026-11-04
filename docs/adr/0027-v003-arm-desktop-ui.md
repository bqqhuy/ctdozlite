# ADR-0027: Giao diện desktop và phiên bản v0.003-arm

- Status: accepted
- Date: 2026-11-04
- Supersedes: ADR-0015 popup lifetime only; extends ADR-0018 and ADR-0019.

## Context

Maintainer đã duyệt sửa theme sáng, bỏ console phụ, hoàn thiện viewer và thumbnail,
clipboard Excel, mention, mở attachment từ cache và updater theo release fork.
Không đổi giao thức đồng bộ, schema hay đưa tài nguyên độc quyền vào sản phẩm.

## Decision

Release label là `v0.003-arm`, định nghĩa duy nhất ở `PRODUCT_VERSION`. Cargo/file
version là `0.0.4`, manifest dùng `0.0.4.0` để giữ định dạng số hợp lệ và tăng so với
EXE cũ. ProductVersion/installer display name/About/CLI dùng nhãn release. Tên
artifact theo nhãn; buildinfo giữ cả hai. Updater so sánh số thứ tự `0.NNN-arm`, vẫn
hiểu tag semver cũ và giữ xác minh checksum/portable ARM64 theo ADR-0019. Bản cũ
không thể tự nhận parser mới; phải cài thủ công một lần.

Release GUI dùng Windows subsystem; chỉ CLI chủ động mới gắn console cha. Không
ẩn console của tiến trình cha. Popup đổi thành 10 giây, giữ khi hover và vẫn có
transition/thời gian mới khi thay thông báo.

Theme phẳng cũ chỉ áp vào dark mode; light mode yêu cầu bảng `light` rõ ràng để
tránh chữ trắng tràn sang nền sáng. Texture tint trắng và các canvas video tối
không bị thay hàng loạt.

Thumbnail dùng kích thước metadata, giữ tỷ lệ và bo góc, không phóng ảnh nhỏ.
Chỉ nhóm ảnh có `photoBundle.groupLayoutId/totalItemInGroup` rõ ràng, cùng người gửi
và hướng gửi; tối đa chín ảnh mỗi khung. Không đoán album theo thời gian gần nhau.

Viewer giữ MFPlay cho video, thêm thanh tua, vị trí/tổng thời gian, hover mốc tua,
trạng thái âm lượng, giữ play/pause sau kéo, DPI và kích thước cửa sổ tối thiểu.
Gallery ảnh/video đọc mục trước/sau trực tiếp từ Store theo `(thread_id, ts, msg_id)`
với LIMIT 1, không hydrate cả lịch sử. Video vẫn phát trong cửa sổ MFPlay riêng;
đóng player để quay về gallery. Lưu/copy dùng đúng message của gallery, kể cả khi
message không nằm trong cửa sổ chat đang tải.

Clipboard ưu tiên PNG/DIB, có EMF dự phòng cho Office. Chỉ giải mã/rasterize sau khi
copy dữ liệu và đóng clipboard. Preview ảnh trước khi gửi; lỗi đọc ảnh phục hồi
văn bản. Ctrl+Shift+V và menu riêng dán chữ. Cài đặt Excel image/text/ask hoạt động
với vùng ô có TSV; ask cho chọn lại dạng chữ trong preview. Kết quả gắn tài khoản
và thread, không chuyển ảnh sang cuộc trò chuyện mới.

Mention chưa đọc có dấu @ riêng; preview chỉ gắn nhãn nếu chính tin preview là
mention. Mở thread không xóa mention chưa thấy; chỉ bỏ dấu của các hàng thực sự
hiện trong viewport, không bị viewer/settings/tools che. Không gửi read receipt.

Mở file tải vào private cache theo tài khoản/thread/message/tên attachment, rồi
mở bằng ứng dụng mặc định. Lưu thành là luồng riêng. Cache không dùng URL làm
khóa, không mở symlink, gộp yêu cầu trùng, giữ ngân sách mục tiêu 512 MiB và tuổi
bảy ngày cho bản hoàn tất không hoạt động. Mỗi file cache tối đa 512 MiB; tệp lớn
hơn dùng Lưu thành. Tệp đang tải được loại khỏi eviction; dung lượng tạm thời có
thể vượt mục tiêu khi nhiều download đang hoạt động. Không giữ mutex khi I/O.

## Evidence

PC 26.11.10: `popup-viewer.html`, module `media-viewer` và các control
`zvideo-player-progress`, `zvideo-player-time`, `zvideo-player-volume` trong
`compact-app-pc.5c0110a3a8749b3aacde.js`. `photoBundle` được tạo với
`groupLayoutId`, `totalItemInGroup`, `isGroupLayout`, `idInGroup` ở byte khoảng
360689 trong bundle trích xuất. Chỉ tham chiếu cấu trúc, không phân phối bundle.

## Verification and rollback

Test parser phiên bản, resource Windows, popup, theme, clipboard DIB, mention,
thumbnail, album metadata, thứ tự gallery/xóa tin, cache và layout viewer. Kiểm tra
API Win32 bằng target ARM64 không thay thế thử nghiệm native thực tế. Quy trình
build bổ sung ảnh chụp viewer/media light/dark. Checklist thiết bị:
[nghiệm thu v0.003-arm](../V0_003_ARM.md).

Không đổi Store schema; rollback code không xóa tin nhắn. Cache attachment là bản
phụ có thể dọn riêng, không phải Store hoặc tệp người dùng chọn Lưu thành.
