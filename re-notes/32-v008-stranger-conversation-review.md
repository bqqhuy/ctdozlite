# Đối chiếu tin người lạ — 05/11/2026

Phạm vi: đọc mã fork v0.007-arm và snapshot Zalo PC 26.11.10/Web đã có trong workspace.
Chưa thử bằng tài khoản thật, không đổi quyền riêng tư, không gửi tin thử trong lượt này.

## Bằng chứng PC/Web

PC: compact-app-pc.5c0110a3a8749b3aacde.js.
Web: 1.c72c0957b455ab70b73c.js. Offset chỉ áp dụng đúng snapshot, không phải API ổn định.

- PC ~9165387: ConvList classifier isInStrangerBox kiểm tra không phải BA account,
  outside state, respondedByMe, group prefix và friend status. Không chỉ dựa danh bạ.
- PC ~9169182…9169407: isStrangerV2 và isInStrangerBoxV2 là hai hàm riêng. Phân nhóm
  conversation có visible/stranger/hidden/outdate. Có ngoại lệ OA, Cloud, pin và cấu hình.
- PC ~19367883 / Web ~14670529: ShowStrangerMsgDirectly có keyName
  setting_show_stranger_msg_directly, settingId 59. Đây là quyền hiển thị, không phải
  bằng chứng rằng tài khoản cho phép nhận mọi tin nhắn người lạ.
- PC ~17158110: service theo dõi thay đổi setting, lấy giá trị user settings. Cùng với
  flag unbox_stranger_thread, setting tác động việc đưa người lạ ra danh sách thường.
- PC ~9141564 và các callsite: shouldApplyNewPrivacyStranger gắn feature flag và
  isStrangerV2; có xử lý riêng cho trình bày seen/typing. Chưa đủ để tuyên bố mọi tin
  người lạ luôn không phát seen hoặc luôn không có thông báo.
- Bảng lỗi có ChattableResultNotAllowStranger/ByOnewayFriend/ChatStrangerBlock và
  ShareFileToStrangerNotAllowed: chattable và loại nội dung có thể bị server hạn chế.
  Chưa xác minh hết request/response và thay đổi state sau thao tác trả lời.

## Fork hiện tại

- src/chat.rs: FriendshipStatus + /api/friend/reqstatus; giao diện có banner chưa kết
  bạn/lời mời gửi/nhận và các thao tác kết bạn. Đây không phải state hộp người lạ.
- SidebarPresentation chưa giữ respondedByMe/outside hay category StrangerBox.
- Luồng ThreadsUpdated nhận live thread mới, tính unread và thông báo theo mute/global;
  không thấy nhánh chính sách riêng cho hộp người lạ. Không chứng minh đã tải đầy đủ
  hộp đó: còn phụ thuộc dữ liệu server giao qua các luồng fork hỗ trợ.
- Composer không bắt buộc kết bạn để gửi cá nhân; server vẫn có thể từ chối. Không có
  quyền vượt chặn server. Hệ ACK/outgoing hiện có cần giữ nguyên khi bổ sung xử lý.
- AppSettings.allow_message_from hiện chỉ thấy khai báo/default, chưa thấy luồng
  đọc/ghi privacy server hoặc consumer thực thi. Không giới thiệu như setting server hoạt động.
- send_typing dùng cài đặt chung và kiểm tra Cloud; chưa thấy phân nhánh stranger-box.
- Chưa thấy luồng block/report chuyên biệt cho hộp người lạ. Từ chối lời mời kết bạn
  khác từ chối/xóa/chặn tin nhắn, không gộp các thao tác này.

## Hướng đang thảo luận, chưa triển khai

1. Tách friendship khỏi inbox classification: Unknown/Normal/StrangerBox. Lỗi tải
   quan hệ/danh bạ không được tự biến thành StrangerBox.
2. Lưu state có bằng chứng, đồng bộ setting hiện trực tiếp và thay đổi nhiều thiết bị.
   Không dùng setting 59 như quyền nhận tin; không tự đổi privacy server.
3. Hộp người lạ có unread/notification phù hợp; reply không đồng nghĩa kết bạn.
   Chuyển hộp chỉ sau ACK/state xác minh, không optimistic theo thao tác mở chat.
4. Thiết kế riêng block/report/delete/request-friend; xác minh hợp đồng trước khi gọi.
5. Fixture và nghiệm thu: bạn bè/người lạ/đã trả lời/OA/nhóm/Cloud, setting trực tiếp
   bật/tắt, bị chặn, gửi file, account switch, offline/reconnect. Không coi thiếu tên
   hay một lần thấy notification là bằng chứng chính sách server.
