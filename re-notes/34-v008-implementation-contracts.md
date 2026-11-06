# v0.008 — hợp đồng triển khai và giới hạn kiểm chứng

Ngày: 05/11/2026 (Asia/Saigon). Working tree trên `feature/v008-arm`, nền `0b1e062`.
Chưa commit/build/release. Fixture dưới đây được dựng từ adapter trong mã chính thức,
không phải capture cuộc gọi của người dùng hoặc xác nhận API bằng tài khoản thật.

## Nguồn

- PC 26.11.10 `compact-app-pc.5c0110a3a8749b3aacde.js`, SHA-256
  `db5966d4b02771f7c32f0884be6be2ec4c9f1b49a1628ae981f535d411150869`.
- Web `1.c72c0957b455ab70b73c.js`, SHA-256
  `29784a88e51221a64f854b059e49e68707ae58cee7a4114bc722ce028a887bf1`.
- Đường dẫn source gốc trong workspace nghiên cứu; không đóng gói lại bundle.

## Adapter nhóm

PC offsets: setting enum ~1740800; rename ~15078900; setting update ~15054148;
blocked list/add/remove ~15055061; link detail/new/disable ~15081159;
pending list/review ~15082654.

- Đổi tên: POST `group/updateinfo`, `{grid,gname,imei}`.
- GET `group/setting/update`: key thật `blockName`, `lockSendMsg`, `lockCreatePoll`,
  `lockCreatePost`, `joinAppr`. Không dùng các tên phỏng đoán `lockName`, `lockAvatar`,
  `membershipApproval`. Field chưa xác minh không được thêm thành toggle.
- Link detail/new có imei; disable không có imei trong adapter.
- Pending list `{grid,imei}`; review `{grid,members,isApprove}`. `users[]` là danh sách
  chờ; phản hồi review có thể có lỗi từng UID, nên outer ACK không chứng minh tất cả đã vào nhóm.
- Blocked list `{grid,page,count,imei}`, response `blocked_members`, `has_more`;
  add/remove `{grid,members}`. UI phân trang 50, tối đa 100 trang; không coi trang đầu là toàn bộ.
- Fixture: [v008-group-requests.json](fixtures/v008-group-requests.json).
- Snapshot quyền và invalidation dùng cùng ID nhóm chuẩn hóa; không giữ cache riêng
  cho `123`, `g123`, `group_123`.

## Liên hệ, lưu trữ và dấu chưa đọc

PC offsets: remove friend ~15072469; block/unblock ~15075920; alias ~15133131;
archive adapter ~15170370, manager ~5218570/5222400; unread adapter ~15070939,
MarkUnreadFetcher ~8551900/8555894.

- Remove và block/unblock là POST friend service `{fid,imei}`, tách reject friend request.
- Alias update GET alias service `{friendId,alias,imei}`; remove GET `{friendId}`.
- Archive list GET label service `{version,imei}`; full snapshot cần `version` và `items`.
  Update POST `{ids:[{id,type}],version,actionType,imei}`; group type 1, direct type 0;
  add action 0, remove action 1. Đọc trước để lấy version và đọc lại sau ACK. Không dùng
  archive để giả lập hide/PIN. `needSync`/version conflicts không tự lặp mutation.
- Manual unread update là POST conversation service, outer `{param: JSON.stringify(inner)}`.
  Add inner `{convsGroup,convsUser,imei}`; mỗi entry `{id,cliMsgId,fromUid,ts}`.
  `ts` là thời gian thao tác hiện tại, không phải thời gian tin nhắn.
  Remove inner có `convsGroup`, `convsUser` (ID arrays) và hai array `*Data` gồm `{id,ts}`.
  Fork yêu cầu định danh tin và người gửi đã biết; không dùng sentinel `-1` như PC.
  Đọc lại full snapshot để reconcile; không phát seen receipt từ thao tác này.

## Cuộc gọi đa thiết bị

PC ~8353349 chuyển nguyên control voip sang native. Source JavaScript này không
chứng minh toàn bộ bảng status trong native. Ghi chú native trước đó ở `07_voip_log.md`
cho thấy callee cũng nhận answer trong lúc thiết lập; không coi mọi answer là self acceptance.

Đã bỏ default status thành 0, yêu cầu ID dương và actor self có namespace UID đã biết
cho accepted-elsewhere. Terminal history bị giới hạn 128 entries/5 phút, theo account
và call engine; GUI có guard riêng chống terminal-before-invite và duplicate invite.
State/identity/incoming events có owner/call ID. Dismiss chỉ drop local state/media,
không đi qua lệnh EndCall để kết thúc cuộc gọi trên điện thoại.

Đây là hardening theo hợp đồng và test mô hình. Chưa có control capture của chính ca
người dùng báo; chưa xác nhận nguyên nhân duy nhất, chưa xác nhận sửa được trên điện thoại.
Diagnostic chỉ ghi act, call ID, status, actor-self và shape của media, không raw payload,
extendData, địa chỉ relay hoặc khóa phiên. Chưa xác minh các act/status native khác.

## Người lạ

`inbox::Metadata` giữ respondedByMe/outside và OA type (chỉ đọc từ profile, không từ
message.type). Merge không biến field thiếu thành false. Candidate không đồng nghĩa
server StrangerBox. Chưa nối setting 59 hoặc tự chuyển/hide hội thoại từ suy đoán.
Friendship response thiếu flags hoặc lỗi lồng nhau là Unknown, không mặc định Stranger.

Xem [ghi chú 32](32-v008-stranger-conversation-review.md) cho phần cần xác minh tiếp.

## Bổ sung đối chiếu các adapter đang triển khai

Các offset sau nằm trong snapshot PC 26.11.10 đã ghi hash ở trên; không phải capture
của tài khoản thật hoặc cam kết API công khai.

- Privacy Web controller ~14678200–14680700: GET setting/me và setting/update với object
  key/value. Chỉ cung cấp các setting có mặt trong snapshot account; đọc lại sau ghi.
- Profile compact ~6376200/6386059: name, gender (0 nam/1 nữ), dob YYYY-MM-DD;
  updateProfileMe ~15167100 gửi social/profile/update, không dùng endpoint đăng ký.
- Avatar compact ~15102090: multipart fileContent, avatarSize160; group có grid/imei.
  Fork tạo cùng một JPEG và bản preview từ file mở một lần; chuẩn hóa EXIF và giới hạn decode.
- InvBox lazy main-startup ~962610/964645/965202: invitations/groupInfo.groupId,
  hasMore/total, join-inv và mdel-inv với invitations stringify; block là lựa chọn riêng.
  enum type0 thường/type1 chờ duyệt, mã 240 không được báo thành đã tham gia.
- Community lazy ~1271766/1272160: canUpgr cho bản thân/người nhận quyền, không lấy
  othersCanUpgr thay selected-member capability. Compact ~3012899: normal1/community2.
- Report compact ~15750500/15754318: user type2/group type14; lý do Khác0; giới hạn
  nội dung 1000 UTF-16. Hiện chỉ có báo cáo user legacy, không gọi là đầy đủ report v2.
- Avatar history lazy main-startup ~1289534/1289751/1290003/1292210/1294562:
  GET avatar-list ban đầu page1, photoId rỗng, albumId0, count50; kết quả albumId,
  hasMore, nextPhotoId, photos[].photoId/thumbnail/url. Reuse giữ wire photoId và
  isPostSocial0; del-avatars dùng delPhotos stringify, photoId dạng chuỗi.
  ACK xóa không đồng nghĩa file CDN đã bị xóa hoặc thay đổi tin nhắn lịch sử.

Regression thêm: hơn 2.000 pending members không bị truncate, bản ghi lỗi không bị bỏ
qua và báo số người thiếu; lịch sử avatar không tiến với cursor đứng yên; reuse không
đăng nhật ký; alias/label vẫn giữ fallback hợp lệ sau serialize/sanitize.

Các thiếu sót còn mở: hidden PIN cần gate toàn bộ search/cache/export/notice; chưa có
thì không dùng local hide để mô phỏng bảo vệ PIN. Presence truy vấn thủ công đã nối UI; chưa có polling/control thời gian thực. Realtime group
control và stranger-box lifecycle chưa hoàn tất; không đánh dấu v0.008 đã nghiệm thu.

### Presence: truy vấn hoạt động gần nhất

Compact ~15152916: GET profile `/api/social/profile/lastOnline` với uid, conv_type,
imei. Consumer lazy default-login-main-startup-shared-worker-znotification ~4275588:
không query khi own show_online_status=false; conv_type1 user,2 OA,0 group; đọc
response.lastOnline và response.settings. Fork hiện chỉ query user/OA khi đã biết loại,
đọc own privacy mới trước mỗi thao tác; missing settings không được diễn giải thành cho phép.
Thiếu/0/âm hoặc timestamp tương lai bất thường là Unavailable, không vẽ chấm online giả.
Đây là hoạt động gần nhất theo server, không phải trạng thái kết nối realtime của người dùng.

### Quyền thêm thành viên

Compact ~13347547 (member picker): canAddMember dựa vào addMemberOnly hoặc joinAppr,
trưởng/phó nhóm được xét riêng. Loại bỏ field suy đoán lockAddMember khỏi guard worker.
Fork thận trọng hơn với snapshot thiếu setting: thành viên thường phải có cả hai giá trị0;
không suy thiếu thành cho phép. Máy chủ vẫn quyết định cuối cùng, không bypass join approval.
Test bao phủ từng cờ, thiếu cờ, field cũ và manager.
