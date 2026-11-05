# v0.006-arm: đối chiếu hợp đồng PC/Web

Ngày: 04/11/2026. Đọc tĩnh Zalo PC 26.11.10 đã trích xuất và bundle công khai Web;
không dùng phiên thật để gọi các API mới. Fixture trong test là dữ liệu tổng hợp.

Nguồn PC: `compact-app-pc.5c0110a3a8749b3aacde.js`; adapter reminder trong
`lazy/main-startup.8b01fad46426bf4cc35b.js`. Offset là chỉ số ký tự Python.
Web chunk `1.c72c0957b455ab70b73c.js`, SHA256
`29784a88e51221a64f854b059e49e68707ae58cee7a4114bc722ce028a887bf1`.

- `getPinnedConversations` tại 15124730: GET CONVERSATION `/api/pinconvers/list`,
  params `{imei}`. Update 12121 gửi `{version,conversations}`. Adapter tại 14550037
  giữ thứ tự, phân biệt g/u, bỏ m1; lỗi 230 phải refetch, không tự ghi đè.
- `sendReactionMessage` tại 15088227 và adapter 11670347: rMsg giữ gMsgID/cMsgID
  và internal msgType của tin nguồn, rIcon/rType/source; REACTION 12410/12411.
  Module 1pet: text 1, photo 2, voice 3, sticker 4, contact 6, GIF 7,
  location 17, video 18, file 19. Đây không phải archive enum.
- `PinMessageMenuItem`, builder `Ae`: MSG_TOPIC=2; params client_msg_id,
  global_msg_id, senderUid, senderName, title và CLI msg_type. CLI text=1,
  photo=32, sticker=36, video=44, file=46, khác reaction enum. Ghim nhóm dùng
  topic/create với needPin=true; unpinv2 gửi topic {topicId,topicType}, boardVersion.
  Không suy đoán hợp đồng ghim tin 1-1 từ ghim nhóm.
- GROUP_BOARD service key là `group_board`. Không đồng nhất với GROUP hoặc BOARD.
  getListPinTopics gửi `{groupId,boardVersion,imei}` tới `/api/board/pin/list`.
- Module BScF và 6lOE: reminder TEXT_TOPIC=0, duration=-1, emoji=⏰,
  needPin=false, REPEAT_NONE=0. Nhóm dùng grid, params là JSON string;
  1-1 dùng toUid, creatorUid trong objectData JSON. Thời điểm UI chuyển sang epoch ms.
  Luồng legacy topic/create/update và oneone/create/update có trong mã chính thức;
  chưa dùng createv2 phụ thuộc feature flag khi chưa xác minh cấu hình phiên.
- Các endpoint đọc: listReminder / oneone/list với board_type=1, page/count/last_id/last_type;
  detail topic/getReminder với eventId. Xóa topic/remove hoặc oneone/remove.
  Adapter loại date/header items khác boardType=1 và chỉ hiển thị reminder startTime>0.
  Mutations không tự retry; reads không được coi missing data là danh sách rỗng.
- PC/Web playNoti tái dùng Audio và throttle 500 ms. Fork vẫn giữ native player;
  chỉ sửa foreground policy, prepare acknowledgement và decoder lifetime.

Cần kiểm chứng thật: routing GROUP_BOARD của phiên Web/QR, các biến thể phản hồi,
quyền nhóm, pin/unpin và reminder hiển thị trên điện thoại, nhận reaction trên các loại tin.
Lỗi phản hồi không được dùng để suy ra tính năng server đã thành công.
