# Bản đồ tính năng/API Zalo PC/Web còn thiếu trong fork

Đây là baseline trước triển khai, không phải danh sách còn thiếu trong working tree v0.008.
Trạng thái hiện tại: [tiến độ v0.008](releases/v0.008-arm-progress.md).

Ngày: 05/11/2026 (Asia/Saigon). Nền đã phát hành: v0.007-arm, commit `0b1e062`.
Toàn bộ nhóm đã được đưa vào [roadmap nhiều đợt](ROADMAP_ZALO_PARITY.md) theo yêu cầu người dùng.
Không gộp toàn bộ vào v0.008.
Phạm vi đã chốt v0.008: [cuộc gọi, UI/privacy, hội thoại/danh bạ/hồ sơ và nhóm/cộng đồng](releases/v0.008-arm-plan.md).

## Cách đọc bằng chứng

- Đối chiếu source Rust và hai snapshot: Zalo PC 26.11.10 compact-app-pc.5c0110a3a8749b3aacde.js;
  Web 1.c72c0957b455ab70b73c.js. Chưa chạy tài khoản thật cho các API mới.
- Quét descriptor và numeric command registration được **421 path /api tĩnh khác nhau**.
  [Index JSON](../re-notes/fixtures/v008-official-route-index.json) có source hash/offset/cmd.
- 119 path có chuỗi trùng trong src Rust, nhưng có thể là comment/test; đường dẫn dựng động,
  WebSocket và service khác chưa được phép so này phản ánh. **Không lấy 421−119 thành số API thiếu,
  không dùng tỷ lệ đó làm phần trăm hoàn thiện.** Index không bao trùm mọi chunk/service Zalo.
- Route/cmd chứng minh endpoint có trong bundle; chưa chứng minh HTTP method, schema, ACK,
  pagination, entitlement hoặc API hiện còn cho tài khoản này dùng. Cần callsite/fixture trước triển khai.
- Không gọi tính năng đã có nền là hoàn toàn thiếu. Chưa có luồng hoàn chỉnh dưới đây là
  chưa thấy implementation/consumer đầy đủ tương đương trong fork ở phạm vi rà soát.

## A. Chưa thấy luồng hoàn chỉnh tương đương

| ID | Tính năng | API/bằng chứng PC/Web | Khoảng trống ở fork |
| :--- | :--- | :--- | :--- |
| A01 | Hộp người lạ và hiện trực tiếp | Setting 59, respondedByMe/outside, ConvList classifier; xem re-notes/32 | Có friendship banner, chưa có inbox classification; đã chốt v0.008 |
| A02 | Chặn/bỏ chặn, danh sách chặn | FRIEND 11422 /api/friend/block; 11433 unblock; 11429 blocklistv2 | Chưa có luồng UI/server chuyên biệt; khác mute và từ chối kết bạn |
| A03 | Báo cáo lạm dụng/spam | PROFILE 15295 /api/report/abuse-v2; 31582 /api/social/profile/reportabuse | Chưa có lựa chọn đối tượng/lý do và mutation đã xác minh |
| A04 | Biệt danh cá nhân đồng bộ | ALIAS 12200 /api/alias/list; 12201 update; 12202 remove; 12205 list/v2 | Chưa có quản lý biệt danh server; khác tên gốc hoặc tên cache |
| A05 | Nhãn/phân loại hội thoại server | 12161 /api/convlabel/get; 12160 /api/convlabel/update | Filter Cá nhân/Nhóm/chưa đọc không phải nhãn tùy chỉnh đồng bộ |
| A06 | Lưu trữ hội thoại server | CONVERSATION 12076 /api/archivedchat/list; 12077 update | Chưa có archive server; khác local hide |
| A07 | Chat ẩn có PIN đồng bộ | CONVERSATION 10048 /api/hiddenconvers/get-all; 10049 add-remove; 10050 update-pin; 10051 reset | Local hidden không phải khóa PIN hay ẩn trên mọi thiết bị |
| A08 | Cập nhật privacy server | 12113 /api/setting/me; 12033 /api/setting/update | allow_message_from chưa có consumer/server flow; cần ánh xạ từng setting, không đoán setting 59 dùng chính API này |
| A09 | Quản lý tài khoản: password/2FA/thiết bị | 12066 /api/update/setting/2FA; 11335 /api/update/password; 11346 /api/login/actlistv2 | Login/logout hiện có không tương đương UI quản lý bảo mật/phiên khác |
| A10 | Hồ sơ/avatar của chính mình | 12075 /api/social/profile/update; 11819 /api/profile/upavatar; 12451 /api/social/avatar-list; 12452 del-avatars; 12453 reuse-avatar | Đọc profile không phải sửa tên/avatar/lịch sử avatar |
| A11 | Danh bạ đầy đủ: xóa bạn, thân thiết, tra cứu bổ sung | FRIEND 11423 /api/friend/remove; 11421 closefr/update; 12128 mget-qr; 10004 search/by-user-name | Có kết bạn/lời mời/tra SĐT, chưa phải toàn bộ quản lý danh bạ |
| A12 | Đổi tên/ảnh và cài đặt nhóm | GROUP 11188 /api/group/updateinfo; 11195 /api/group/upavatar; 11816 /api/group/setting/update | Quản trị thành viên đã có; chưa có UI đầy đủ chỉnh thông tin/quy định nhóm |
| A13 | Link mời nhóm, tạo mới/vô hiệu hóa và tham gia | GROUP 11821…11825 /api/group/link/detail,new,disable,ginfo,join | Chưa có quản lý link mời/QR/tham gia tương đương |
| A14 | Duyệt thành viên và danh sách cấm nhóm | GROUP 10064 /api/group/pending-mems/list; 10065 review; 11811…11813 /api/group/blockedmems/list,add,remove | Invite/kick không thay duyệt tham gia và ban/unban |
| A15 | Hộp lời mời nhóm/cộng đồng và nâng cấp | GROUP 12170…12173 /api/group/inv-box/*; 12180 /api/group/upgrade/community; 12181/12182 quota | Chưa có workflow/entitlement/quota/duyệt cộng đồng đầy đủ |
| A16 | Ghi âm và gửi tin thoại | FILE 11148 /api/message/voicemp3; 11169 /api/group/sendmp3; 12060/12063 voice/upload | Nhận diện chat.voice và âm thanh cuộc gọi không phải recorder + gửi voice message |
| A17 | Chia sẻ danh thiếp | CHAT 11774 /api/message/contact; GROUP 11827 /api/group/contact | Nhận diện chat.contact chưa chứng minh gửi/hiển thị card đầy đủ |
| A18 | Giao việc/to-do có trạng thái | 12413/12414 /api/board/personal/todo/create,update; 12416 personal/list; 12418 todo/status | Ghi chú, nhắc hẹn và lịch gửi local không thay task có người nhận/trạng thái |
| A19 | Chat mã hóa đầu-cuối E2EE | CHAT_E2EE/GROUP_E2EE: 160xx/161xx /api/e2ee/pc/message/* và group/* | HTTPS/ParamCipher và khóa Sync2/V1 không phải E2EE hội thoại; chưa có key/session lifecycle chat tương ứng |
| A20 | zCloud media/khôi phục dữ liệu cloud | /api/getCloudViewerKey; /cloudmedia/queue/pc/verify; /cloudmedia/downloadurls/pc/v1 (ngoài index numeric) | Cloud của tôi, renewlink, Sync2/V1 và backup local không phải zCloud viewer/decryption; cần thử một file thật được phép |

## B. Đã có nền, còn thiếu chiều sâu hoặc đồng bộ server

| ID | Tính năng | Nền hiện có | Còn thiếu / API liên quan |
| :--- | :--- | :--- | :--- |
| B01 | Đánh dấu chưa đọc chủ động, đồng bộ nhiều thiết bị | Nhận unread/read cursor, filter và local unread | CONVERSATION 12013 addUnreadMark /12014 removeUnreadMark; getter getUnreadMark có sẵn; khác chỉ mở hội thoại |
| B02 | Delivered/seen/typing đầy đủ | Socket ACK, nhận receipts và gửi typing | HTTP seen/delivered/getStatusMsg có route riêng; không mặc định phải thêm tất cả. Chưa đối chiếu đầy đủ policy người lạ/nhóm; gửi seen cần lựa chọn riêng tư rõ |
| B03 | Quyền cộng đồng/control realtime | lockSendMsg, creator/admin, kiểm tra outgoing và UI | Chưa đủ banned/approval/disband/entitlement và invalidation tức thời; API thêm thuộc A12…A15 |
| B04 | Poll/note/reminder nâng cao | CRUD note text, poll vote/add/end, nhắc hẹn một lần | POLL 12085 /api/poll/share; BOARD 12086 listResponseEvent/12087 updateEventResponse; thiếu RSVP/lặp/chi tiết voter/rich board đầy đủ |
| B05 | Kho ảnh/video/file server | Tìm toàn DB local, lọc sender/date/extension | 11841…11843 /api/mediastore/list,preview,remove; 12220…12222 /api/media/oneone/list,preview,remove; không tự suy đây là toàn lịch sử hoặc zCloud |
| B06 | Forward/thu hồi nhiều tin một lượt | Forward có source protection, recall từng tin có ACK | 12440 /api/message/mforward; 12442 /api/group/mforward; 12115 multi_undo; 12116 group/multi_undomsg; cần UI selection và hợp đồng batch |
| B07 | Tin nhanh/auto-reply đồng bộ | Quick/editor và reply rules local | 12025…12032 /api/quickmessage/list,create,update,delete; 12048…12051 /api/autoreply/*; khác rules chỉ chạy khi fork đang mở |
| B08 | GIF/sticker/effect cá nhân hóa | Sticker/GIF nền, Ami, suggestion hiện có | Search/recommended/personalized/AI/Tenor/Giphy/effects chưa đủ tương đương; ví dụ 11747 search,11748 recommended,11745/11746 personalized |
| B09 | Chia sẻ màn hình cuộc gọi | Signaling 12064 /api/voicecall/requestsharescreen và 12044 group/sharescreen có trong call.rs | Chưa thấy pipeline capture/encode/transmit màn hình và UI đã nghiệm thu; screenshot gửi chat là luồng khác |
| B10 | Presence/last online có server | Có typing, trạng thái UI/roster | PROFILE 11290 /api/social/friend/onlines; 12090 /api/social/profile/lastOnline; chưa xác nhận luồng đầy đủ và privacy; không tin nhãn tĩnh là dữ liệu live |
| B11 | V1/backup và media lịch sử | Có Sync2, V1 chủ động, Desktop V2/JXL thử nghiệm | BinNet/media V1, zCloud và các case iPhone/account/cancel còn thiếu; không thay bằng suy luận sender thống kê |
| B12 | Trình xem media và UX | Có timeline video, gallery, zoom/save/copy, native player | Chưa tương đương toàn UX PC; không gọi là hoàn toàn thiếu viewer. Streaming/tua preview/đa cửa sổ cần đối chiếu thêm, không có route đã xác minh để tự triển khai |

## C. Có dấu vết API, chưa kết luận đủ về tính năng/khả dụng

- C01 — Business/OA/enterprise/product catalog: /api/prodcatalog/*, enterprise profiles,
  OA block/unblock, trial/subscription. Tùy entitlement; không đưa thành chức năng phổ thông.
- C02 — Đăng ký/khôi phục tài khoản/đổi mode: có các route register, reset password, ekyc,
  nhưng không tự gọi hoặc coi tương đương toàn mobile app.
- C03 — /api/lan/* và /api/transfer/*: có bảng lệnh, chưa xác minh đủ workflow; không tự gọi
  là chuyển tiền/ZaloPay hay truyền file LAN hoàn chỉnh chỉ từ tên API.
- C04 — Các route legacy/v2 song song không phải hai tính năng riêng. Một route PC có thể không
  hợp lệ với web session/service map của fork; endpoint phải resolve đúng service.

## Những phần đã có, không thống kê là hoàn toàn thiếu

Gửi/nhận text và file/ảnh/video; quote/jump/forward; reaction; typing/receipt nền;
tìm lịch sử local và thư viện có filter; server pin/reminder; nhóm cơ bản/poll/note/TTL;
media renewlink/cache; mute/unread; audio/video call và nhóm nền; Sync2 và fallback thử nghiệm.
Từng phần vẫn có giới hạn và task chưa hoàn tất, không suy thành tương đương Zalo PC.

## Ưu tiên đề xuất sau phạm vi đã chốt

1. Nhất quán và riêng tư: A01…A03/A08; B01…B03. Hoàn thiện chức năng đang có trước.
2. Công cụ hội thoại: A04…A07, A12…A14, B04…B07 — giá trị cao, có nhiều bằng chứng route.
3. Nội dung mới: A16…A18, UI/viewer/presence; cần nghiệm thu thiết bị.
4. Nghiên cứu riêng: A19/A20, phần Business/LAN; không trộn vào một bản nhỏ khi
   thiếu quản lý khóa/entitlement/capture. Không cam kết khôi phục media chỉ vì có route.

Đây là đề xuất ưu tiên, không phải lệnh thực thi toàn bộ hoặc thay đổi privacy mặc định.
