# v0.008: lifecycle, PIN và liên hệ nội dung — 06/11/2026

Nguồn PC/Web dùng cùng hash ở re-notes/34. Đây là source-derived evidence, không phải
capture tài khoản thật. Không chứa tin nhắn/cookie/key thật.

## Control

PC compact ~14581700–14584200: group, fr, alias/profile, label_convers, hidden_chat,
my_settings và mark_unread. `_processGroupCtrlEvents_old` ~14588688 dùng groupId/group_id;
update_setting/add_admin/remove_admin làm mất hiệu lực metadata. `_processProfileCtrl`
~14585657 có online_status {uid,lastOnline}, tách khỏi update/biz/mode.

Fork chỉ parse từ các queue control xác định bởi socket::WsEvent::is_voip_ish; text chat
không được biến thành control. WS và HTTP polling dùng chung Impact, không truyền raw
PIN/metadata vào log. Online status không kéo toàn bộ account metadata. Group invalidation
xóa ACL ngay, refresh đúng nhóm; role cache dùng runtime generation duy nhất và revision.
ACL không serialize vào SidebarPresentation; membership version không chứng minh ACL còn đúng.

## Hidden chat

PC compact ~4798100–4813000: map {thread_id:bare,is_group:0/1}; get-all trả threads và
pin; update-pin dùng MD5 của bốn số; add-remove stringify hai mảng. `change_pin`/add_remove
control làm thay state. Fork dùng cùng protocol nhưng không lưu verifier vào đĩa. Thiếu pin
không trở thành pin rỗng: vẫn giữ membership và chỉ cho đọc các chat không ẩn.

Cache membership và cache policy/alias có owner/version, canonical IDs, giới hạn kích thước;
không lưu PIN, verifier, khóa hay nội dung tin. Cache dùng đường dẫn account_file hiện có.
Offline không unlock chat ẩn. Không triển khai reset phá hủy theo suy đoán; xem ADR-0034.

## Danh bạ và bạn thân

PC compact ~15166230/10638828: mget-qr POST {fids:[UID]}, map UID→QR URL.
Tra username: route 10004 có trong PC/Web; schema tham chiếu zca-js commit a1283b0,
`src/apis/findUserByUsername.ts`: GET friend/search/by-user-name {user_name,avatar_size:240};
`UserBasic`: uid/display_name/zalo_name. Đây là nguồn thứ cấp, vẫn cần thử tài khoản thật.
Đọc bạn thân: getclosedfriends, profile service; PC ~15078421 và zca-js getCloseFriends.ts.

Ghi bạn thân không bật vì có bằng chứng disabled/no-op trong chính PC 26.11.10:
~8053829 isOffCloseFriendFeature=true; ~11793815 `_toogleCloseFriend(){}`;
~18871886 ADD_CLOSE_FRIEND/REMOVE_CLOSE_FRIEND chỉ break. Wrapper closefr/update còn tồn tại
nhưng không chứng minh enum action hoặc luồng đang hoạt động. Không tự đoán 0/1 để gọi ghi.

Báo cáo nhóm: PC ~15753800–15754900 bỏ group prefix khỏi uidTo, type14; user type2;
Other reason0. Fork không gộp report với block/leave/delete. Report v2/screenshot ngoài luồng này.

## Poll/ghi chú

Nguồn board/list và cursor ở re-notes/31. PollDetail model của PC và zca-js models/Board.ts
không bảo đảm có group_id. Vì vậy không dùng group của cửa sổ hoặc creator do UI truyền
để cấp quyền. Fork xác minh item qua board/list với group_id; binding theo account/runtime/
revision, TTL 5 phút, tối đa 2048; tìm kiếm có hạn 15 giây/200 trang, cursor không lặp.
Detail phải khớp poll_id đã yêu cầu. Note sửa/xóa dùng creator từ item server.
Không tìm thấy/bị cắt/timeout là chưa xác minh, không đồng nghĩa nội dung đã bị xóa.

## Hiệu năng và giới hạn

Metadata không nhập lại tin lịch sử; cache policy và alias giữ trạng thái khi offline.
Chu kỳ khi yên lặng 15 phút, có wake từ control/reconnect/người gửi chưa nhận diện.
Chỉ persist thread khi metadata thay đổi. Poll scope scan có cache ngắn hạn, không tự lặp mutation.
Không thay worker, import/checkpoint, codec hoặc schema SQLite. Native/điện thoại vẫn cần nghiệm thu.
