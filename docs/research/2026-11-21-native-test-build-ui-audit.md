# Audit trực tiếp bản test Windows ARM64 — UI, UX và phản hồi gửi

Ngày: 21/11/2026, Asia/Saigon. Trạng thái: đã build, chạy và quan sát bản test; còn các phép đo và luồng chưa nghiệm thu.

## 1. Binary và điều kiện

- Lệnh: `cargo build --locked --bin ctdozlite -j 1` — thành công, 2 phút 15 giây.
- Binary: `C:\Projects\ZaloLite\target\debug\ctdozlite.exe`, 88.227.840 byte. Đây là bản debug ARM64, không phải release benchmark.
- SHA-256: `16953D5366D388628E08B8BB1D7678FA594BA72CAE569527EC31DE4579398294`.
- HEAD nền: `8e235f764e0096cdbf9576567589c7c13bc3378d` cộng working-tree P0/P1 chưa commit.
- Thiết bị: Snapdragon 8cx Gen 3, 8 logical processors. Phiên hiện có được khôi phục; không thao tác đăng nhập/logout hoặc chuyển account trong đợt này.
- Cửa sổ quan sát khoảng 1023×732 và 1707×1019 theo ảnh công cụ. Đây là kích thước ảnh, không khẳng định physical resolution/DPI. UI scale trong Cài đặt hiển thị 1.00, font Be Vietnam Pro, light theme.
- Mở chat cá nhân Henry, Cloud và nhóm; kiểm tra search, drawer, danh bạ bạn bè/nhóm, sticker/emoji, Cài đặt chung/Đồng bộ/Giao diện, composer, file/image picker. Gửi nội dung tổng hợp chỉ tới Cloud của chính người dùng.
- Không lưu screenshot chat riêng, nội dung hội thoại, UID, cookie, session hay URL tin nhắn vào repository. Bằng chứng visual là ảnh trực tiếp trong lượt công cụ; tọa độ dưới đây là đo xấp xỉ từ ảnh.

## 2. Kết luận độc lập

**Giao diện còn nhiều vấn đề thật, gồm lỗi layout cản trở sử dụng và thiếu tính đồng nhất giữa các surface.** Cảm giác rối không chỉ do có nhiều tính năng: cùng loại thao tác được trình bày bằng nhiều kiểu nút, mức ưu tiên thị giác chưa rõ, popup không bám vùng thao tác, và những vùng rộng/rỗng tồn tại cạnh vùng bị ép hẹp.

Không phải mọi vùng đều xấu: nav rail có nhịp ổn định; Cài đặt chung có grouping/card khá rõ; văn bản tiếng Việt cơ bản đọc được; bubble text và reaction asset màu có thể hiển thị bình thường. Những điểm này nên giữ và dùng làm chuẩn, thay vì thay toàn bộ giao diện theo cảm tính.

Các phase GUI được ghi merged trong tài liệu cũ chỉ phản ánh lịch sử implementation. Chúng không chứng nhận visual acceptance của binary hiện tại; cũng không được nhầm với hai audit P1 về identity/barrier.

## 3. Findings trực tiếp

Mức A: cản trở thao tác/đọc hoặc có rủi ro gửi ngoài ý muốn. Mức B: sai lệch lớn về bố cục và tính nhất quán. Mức C: polish. `OBS` là quan sát native; `AUTO` có khả năng bị ảnh hưởng bởi input automation; `CODE` là đối chiếu source.

| ID | Mức | Bằng chứng và vấn đề | Hướng xử lý |
|---|---|---|---|
| N01 | A | OBS: dải đen liên tục giữa sidebar và chat, khoảng 107 px ở ảnh 1023 px (x≈371–478), khoảng 96 px ở ảnh 1707 px (x≈484–580). Có từ trước search, còn sau search, ở nhiều chat; không có trong trang Danh bạ. | Sửa allocation/painting/clipping của workspace; đo rect panel/content. Chưa chốt nguyên nhân từ ảnh hoặc gọi là lỗi GPU. |
| N02 | A | OBS: search và hàng filter bị cắt tại mép sidebar; Phân loại/menu và action cuối không hiện trọn. Sidebar dùng diện tích nhưng content vẫn ngắn/hẹp. | Đặt budget cố định cho action, input lấy phần còn lại; label truncate theo budget; kiểm tra cùng banner đang hiện/ẩn. |
| N03 | A | OBS: có banner báo chưa lưu được cập nhật vào CSDL và giữ cursor cũ; tồn tại suốt nhiều lượt quan sát. | Điều tra lỗi persistence riêng, không xử lý như lỗi màu sắc. Giữ trạng thái partial thật và cung cấp hành động khôi phục dễ hiểu. |
| N04 | A | AUTO: hai lần Shift+Enter gửi nội dung Cloud thay vì xuống dòng; Ctrl+Shift+S với hai cách gọi modifier không mở formatting. CODE: source có xử lý shortcut và Shift guard. | Kiểm tra event-level modifiers cùng native input/human reproduction; không coi sự có mặt của code là đã sửa, cũng không khẳng định công cụ không thể gây sai lệch modifier. |
| N05 | A | AUTO: 5 intent bàn phím k1…k5 trong 1.381 ms đến snapshot tạo ba bubble `k1`, `k2k3k`, `4k5`. Lượt typeText trước đó cũng trộn nội dung. | Chưa đạt bài kiểm thử burst. Rà xử lý nhiều Enter/text events trong cùng frame và giới hạn input harness; phân biệt mất ranh giới composer với network QueueWait. Không kết luận server đã mất tin. |
| N06 | A | OBS/AUTO: click file không thấy dialog; click ảnh sau đó báo hộp chọn tệp đang mở, nhưng inventory và snapshot không tìm được dialog. | Xác minh native picker STA/ownership/foreground/lifecycle; không ghi đã gửi file/ảnh. Source dùng rfd IFileOpenDialog, không phải nút stub đã chứng minh. |
| N07 | B | OBS: thẻ cuộc gọi Henry bị ép phần text cực hẹp, xuống dòng gần từng âm tiết, trong khi bubble/card còn chỗ; timestamp nội/ngoại lặp. CODE: call card horizontal gồm icon, vertical text và Gọi lại. | Dành min width cho text/call card, tách action theo budget; dùng một metadata row thống nhất. |
| N08 | B | OBS: sticker popup mở sang phía sidebar (x≈175–635), trong khi nút mở ở composer x≈620; chồng lên sidebar/dải đen. Nhiều hàng tab, pack, search và hai nút đóng cùng xuất hiện; pack cuối bị cắt. | Neo popup vào composer và clamp theo viewport; một navigation hierarchy và một affordance đóng; overflow pack có chủ ý. |
| N09 | B | OBS: avatar trong danh bạ bạn bè/nhóm dùng initials đồng loạt, còn sidebar và header dùng ảnh thật/composite. CODE: contacts.rs gọi avatar_initials ở các row. | Một renderer/avatar model dùng chung giữa sidebar, contacts, drawer; fallback chỉ khi thiếu ảnh thật. |
| N10 | B | OBS: danh bạ toàn chiều rộng với tên sát trái, Nhắn tin ở tận mép phải, khoảng trắng rất lớn; caption Bạn bè/Nhóm lặp ở mọi dòng. Badge 99+ không hữu ích khi trang có tổng số cụ thể. | Chuẩn row width/action placement và metadata cần thiết; phân biệt tổng số với unread count. |
| N11 | B | OBS: header lặp Phân loại/badge; drawer cá nhân có bốn action, cả Tạo nhóm và Cài đặt; Cloud vẫn có icon gọi thoại/video. | Ma trận action theo loại thread; thao tác chính luôn ở vị trí ổn định, thao tác hiếm vào menu, đặt tên rõ theo tác dụng. Không tự bỏ tính năng. |
| N12 | B | OBS: drawer Cloud và trang Đồng bộ có nhiều nút nhỏ rộng theo chữ, xếp nối tiếp; Cài đặt chung dùng card/toggle rõ hơn; typography/spacing và hierarchy không thống nhất. | Dùng chung control heights, typography và section/action groups; một primary action cho mỗi flow. |
| N13 | B | OBS: trang Đồng bộ đưa SQLite, giới hạn 200 trang/hội thoại và nhiều cách tải dữ liệu lên bề mặt mặc định; khó chọn thao tác phù hợp. | Tách lựa chọn theo mục tiêu người dùng và trạng thái hiện tại; giữ chi tiết kỹ thuật trong mục mở rộng, không xóa chẩn đoán cần thiết. |
| N14 | B | OBS: tên có heart hiện đơn sắc và khoảng cách glyph lớn, trong khi reaction/emoji picker có asset màu. Có sự khác nhau giữa rendering user text và icon/asset. | Kiểm tra grapheme/VS/ZWJ và pipeline emoji text; không dùng khoảng trắng hoặc sửa tên để che lỗi. Không coi mọi emoji trong app đều đen. |
| N15 | B | OBS: accessibility tree chỉ có cửa sổ/title bar và ba nút OS; không thấy control/chat trong app. | Xác minh cấu hình accessibility/egui AccessKit và keyboard navigation. Hiện automation/screen-reader không có semantic tree hữu dụng; chưa khẳng định mọi screen reader đều thất bại. |
| N16 | C | OBS: composer khoảng 100 px cuối cửa sổ gồm toolbar, editor và margin; editor text nhìn sát mép trên, metadata/button dùng cỡ khác nhau. | Chuẩn baseline, vertical alignment và min/multiline height. Khoảng trống dưới composer không được khẳng định là quá lớn ở mọi trạng thái: ảnh mặc định hiện chỉ còn margin nhỏ ở đáy. |

## 4. Đo phản hồi và tài nguyên

| Kịch bản | Kết quả thật | Giới hạn |
|---|---|---|
| Một tin Cloud đã xác nhận nội dung trước Enter | Bubble + Đã gửi có trong snapshot sau 848 ms | Includes input/snapshot/tool overhead; không tách Enter→Pending hoặc transport ACK |
| Một tin Cloud bằng phím trực tiếp và xác nhận trước Enter | Bubble + Đã gửi trong snapshot sau 1.014 ms | Như trên; n=2 không đủ p50/p95, không kiểm chứng phía server bằng client thứ hai |
| Burst 5 lần typeText/Enter | Lượt input+snapshot 3.226 ms; nội dung trộn/lặp | Loại khỏi benchmark transport vì clipboard/input harness chưa kiểm soát |
| Burst 5 lần phím trực tiếp k1…k5 | Lượt input+snapshot 1.381 ms; ba bubble với ranh giới sai | Finding N05; chưa thể phân biệt toàn bộ phần đóng góp của input harness và frame handling |
| Nhận tin tự nhiên | Sidebar preview và unread/title thay đổi khi không thao tác | Không có nguồn phát kiểm soát/timestamp ingress, không đo inbound latency hoặc cam kết history không chặn realtime |
| Idle UI 10,009 giây, Cloud mở, không input | CPU 11,016 giây = 110,06% một core = 13,76% tổng 8 logical; working set 1.329,59 MiB; private 1.211,73 MiB | Bản debug; background traffic/data thật chưa kiểm soát; một mẫu không chứng minh memory leak hoặc mức release |

Không có số đo frame p95, queue depth, DB busy/transaction duration hoặc request→ACK trong đợt này. Timing ring hiện nằm trong RAM; đo UI automation không thay instrumentation. Debug build không dùng để kết luận release chậm hơn Zalo PC bao nhiêu phần trăm.

## 5. Phần chưa nghiệm thu

- Upload/download file, ảnh, video: bị chặn tại native picker chưa thấy; chưa có kết quả end-to-end. Hai fixture tổng hợp TXT/PNG được tạo trong thư mục tạm, chưa upload.
- Cuộc gọi đến/đi: chưa gọi, không xác nhận codec/device/signalling.
- Group sending permission: mở nhóm và đọc timeline; không gửi vào nhóm. Chưa kết luận lỗi treo quyền gửi đã hết.
- Human IME, keyboard modifier, screen reader, dark theme và nhiều DPI: chưa được nghiệm thu.
- Cửa sổ 1023×732 và maximized đã quan sát; drag resize không thay đổi kích thước nên chưa nghiệm thu breakpoint cửa sổ nhỏ hơn.
- Zalo PC gốc không được mở/đo cùng workload trong lượt này; tài liệu khảo sát 17/11 chỉ là tham chiếu, không phải đối chứng live hôm nay.
- Hai audit P1 Accept và unit tests vẫn là bằng chứng source. Các findings native mới phải được giữ mở, không suy ra sản phẩm đã hoàn chỉnh.

## 6. Thứ tự sửa đề xuất

1. **Chặn lỗi vận hành:** CSDL/cursor warning, native picker, ranh giới Enter/Shift+Enter/burst. Thêm timing allowlist cho frame, admission và persistence trước khi kết luận nút thắt.
2. **Sửa layout nền:** dải đen, width budget sidebar/header/composer/call cards và popup anchoring; kiểm tra banner hiện/ẩn, drawer dock/overlay.
3. **Thống nhất component:** control heights và spacing 4/8/12/16; primary/secondary/tertiary button rõ; common avatar, status, section header và metadata row; dùng theme tokens chung.
4. **Sắp lại thông tin:** action theo loại thread, làm gọn sticker, drawer và sync; chuyển action hiếm/diagnostics vào đúng nơi.
5. **Nghiệm thu bằng binary mới:** visual matrix light/dark, viewport nhỏ/vừa/lớn, DPI; kiểm tra lại send/receive/media với sample đủ lớn. Chỉ đóng từng finding khi có kết quả thật.

Đợt audit này không sửa mã giao diện, không commit/push/release. Bản test được để mở tại Cloud để người dùng kiểm tra.
