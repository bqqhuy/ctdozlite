# Phân tích trực tiếp Zalo PC, Web, Android và ZaloData

Ngày: 22/11/2026 (Asia/Saigon). Phạm vi: truy lại popup Phân loại, tên
alias/phonebook, voice và composer từ nguồn gốc, sau yêu cầu người dùng phân biệt
công việc trực tiếp của phiên này với ghi chú kế thừa.

**Kết luận:** đã tự trích ASAR, tải bundle Web hiện được bootstrap công khai tham
chiếu, giải mã/disassemble DEX Android và đọc metadata/schema ZaloData. Tìm thấy
các nhánh alias v2 và composer legacy/column chưa được thể hiện đầy đủ trong fork.
Không thể coi bốn bản sửa hiện tại là parity hoàn chỉnh từ các test đã qua.

Đây là **E2 — phân tích tĩnh**, cộng quan sát cấu trúc dữ liệu cục bộ. Không có
capture API đã đăng nhập, không xác định feature flag thực tế của tài khoản và
không chứng nhận server chấp nhận request. Trong lượt này không sửa Rust,
commit, push hoặc thay dữ liệu Zalo gốc.

## 1. Nguồn được đọc trực tiếp

| Nguồn | Thao tác thực hiện trong lượt này | Định danh |
| :--- | :--- | :--- |
| PC | Đọc `Zalo-26.11.12/resources/app.asar`, parse header/index, tự trích 109 file JS/CSS/HTML/package từ 13.169 entry | `package.json`: Zalo 26.11.12; ASAR SHA-256 `4c3a7aa9b8fb409a95c6739ef9dccdc01127d0dc4f6383ee396400b7f94484c5` |
| Web | GET trang công khai, đọc runtime webpack để lấy đúng tên/hash lazy chunk, tải 13 script, không dùng cookie tài khoản | [Trang gốc](https://chat.zalo.me/); [bundle chính](https://zalo-chat-static.zadn.vn/v1/lazy/1.c72c0957b455ab70b73c.js), SHA-256 `29784a88e51221a64f854b059e49e68707ae58cee7a4114bc722ce028a887bf1` |
| Android | Mở APKM người dùng cung cấp; trích `base.apk` và sáu DEX; đọc AndroidManifest và disassemble các method được truy ra | `com.zing.zalo`, 26.11.01, code 261101901, min SDK 24; APKM SHA-256 `8a16f9616ed52b9b41d02579d3f8e26c947160e777af3403aaa70a78d1bac1c6` |
| ZaloData | Chỉ đọc tên khóa/config, cấu trúc file, header DB và schema của một số DB dùng chung | Thư mục do người dùng chỉ định; không xuất giá trị khóa mã hóa, UID, contact hay nội dung chat |

PC bundle chính SHA-256
`78801014154a86bade096eb916e541223a5f553f888882aa0ef3a218a44d6c08`.
Lazy PC `main-startup.41abf8e6af2a55551e3b.js` SHA-256
`c29c9f0a7f0718c5b044764666b6fc5dec8b8eb65e178819b3cee3b1f93e3426`.

[Index bằng chứng](2026-11-22-direct-analysis-evidence.json) lưu hash, URL, 24
điểm neo và descriptor method Android. Offset trong phần dưới là **code point
Python, zero-based**; index lưu riêng UTF-16 và byte UTF-8 để tránh nhập nhằng
với offset ghi bằng .NET trong ghi chú trước.

Artifact đọc lại ở `%TEMP%/zalolite-direct-audit-20261122/`: source trích mới,
manifest, `scan_dex.py` và các file `*-smali.txt`. Không đưa bundle/APK/DB vào Git.

## 2. Alias và pbName: phải phân biệt v1 với v2

### Luồng phonebook cũ

PC `static getPhonebookV2` (15.120.390) thực sự dựng GET đến
`/api/social/friend/phonebook/v2`, command 12428, params session-AES chứa
`phonebook_version`; giá trị mặc định của hàm là 0. Đây là đọc implementation,
không suy schema từ route index.

Manager `UUq1` gọi bằng phiên bản đang lưu, đọc `version`/`phonebook` từ kết quả
và thay snapshot UID → tên trong `_updateDataPhonebook` (10.569.546). Worker cache
`JWY6.getMiniInfo` và `getDName` thử alias, tên phonebook rồi tên Zalo. Cache cũ
`getInfo` còn áp dụng `enable_feature_alias` và điều kiện bạn bè/stranger alias.

### Nhánh v2 là khoảng trống quan trọng

- `static getAliasV2` (15.132.619) gọi `/api/alias/list/v2`, command 12205,
  params gồm `page`, `count`, `src`, `imei`.
- `_callApi` trong alias manager chọn `getAliasV2` hoặc `getAlias` theo
  `enable_feature_alias_v2`; vẫn kiểm tra `enable_feature_alias`.
- `_startFlowFetchPhonebook` (10.571.146) **không chạy nhánh phonebook cũ khi
  alias v2 bật**.
- `JWY6.getPhonebookName` (6.884.823) trả `null` khi alias v2 bật.
- Nhánh alias v2 có periodic sync và retry policy riêng; `_updateDataAlias`
  (11.639.856) đọc các row `userId`/`alias`, thay cache và tính UID thay đổi.

Đã đối chiếu lại độc lập trong Web tải mới: `getAliasV2` (11.663.912),
`getPhonebookV2` (11.651.683), `getPhonebookName` (5.587.170). Không chỉ có PC.

**Hệ quả cho fork:** `src/account_metadata.rs` hiện gọi `/api/alias/list` và lấy
phonebook version 0 riêng. Bộ ưu tiên tên đã sửa là hữu ích cho nhánh cũ; chưa
có lựa chọn alias v2 theo cấu hình phiên. Chưa biết tài khoản người dùng bật
nhánh nào, nên chưa thể kết luận đây chính là nguyên nhân tên thật còn sai.
Không tự thêm request v2 hoặc gộp nguồn khi chưa nối rõ feature/owner/runtime.

### Bằng chứng Android ở mức bytecode

Đã lần từ chuỗi log `AliasManager` đến lớp obfuscated `Lix;` trong `classes3.dex`:

- `o(Object)` kiểm tra feature guard rồi đọc `JSONArray` có `userId`/`alias`.
  Log gắn luồng này với `Cmd 600 Type 183`; không dùng ID Android này cho PC.
- `r(String,String,boolean)` xử lý save/delete; nhánh xóa gọi SQL với **hai
  tham số** `currentUserUid` và `uIdTo` (instruction 48–52).
- DEX có schema literal `alias_friend(currentUserUid,uIdTo,aliasName,opt_1,opt_2)`.
  SQL literal là bằng chứng tĩnh; chưa mở DB Android thật.

Điều này xác nhận alias cần owner scope và luồng cập nhật riêng, không chỉ đổi
chuỗi tên hiển thị của một group message.

## 3. Composer: ba khái niệm hình học khác nhau

### Legacy normal/rich

Module PC `WJhq` (11.657.383), được Web xác nhận ở 9.227.842, khai báo:

- line height cơ sở 22,5 px;
- normal min một dòng, max `5.5 × 22.5 = 123.75 px`;
- rich min cùng mốc `5.5 × 22.5`, max `auto`;
- tỷ lệ mở rộng `2/3`.

CSS PC khớp: `.chat-input-container .chat-input-scrollable` có max-height
123,75 px; rich mini container có min/max/height 177,75 px. Các con số thuộc
những box khác nhau, không thay trực tiếp cho nhau. Mốc 5,5 dòng trong mã phù
hợp với khả năng người dùng thấy khoảng 5 dòng trọn vẹn, nhưng đó là diễn giải
hình học, chưa phải đo UI native trong lượt này.

### Remote config và nhánh column

`gpmR` (15.579.158) có default version 1, `lineHeight=22`,
`inputMaxLines=12.5`, `rtfInputMinLines=1` và các feature mặc định tắt.
`HKtk.getRemoteConfigPerSession` (6.124.434) lấy snapshot hợp lệ cho phiên;
`WmaB` (11.723.997) chọn editor/layout theo feature flags:

| Điều kiện chính | Kết quả trong bundle |
| :--- | :--- |
| `rtfInput`, `plainInput` bật và không bị disable | Plain box có thể dùng `zcinput`; nếu không thì `v3` |
| `rtfInput` bật | Rich box dùng `zcinput`; nếu không thì `v4` |
| `inputColumnLayout`, `rtfInput`, `plainInput` bật và không bị disable | Layout `column`; nếu không thì `legacy` |

Trong `/Lbf.writeColumnMetrics`, code đọc số dòng/line height của phiên và ghi
CSS variables; expanded height bằng chiều cao column nhân export `WJhq.a`,
tức **2/3**. CSS có fallback 66,667%, đồng thời clamp theo min height và 100%.
`inputMaxLines × lineHeight` là budget text, không phải tổng chiều cao composer.

**Không phủ nhận quan sát “gần một nửa” của người dùng:** column, timeline,
toolbar và toàn cửa sổ có mẫu số khác nhau. Bằng chứng mới chỉ bác việc gọi
1/2 là hằng số phổ quát hoặc gọi 12,5 là giới hạn của mọi editor Zalo.

**Hệ quả cho fork:** `src/composer_config.rs` mới đọc hai trường geometry;
`src/gui/workspace.rs` cố định plain 5 dòng và expanded khoảng nửa vùng chat.
Đây là lựa chọn bám quan sát người dùng, chưa parity đầy đủ rollout/layout của
PC/Web. Cần biết nhánh editor đang dùng trước khi điều chỉnh tiếp các con số.

## 4. Voice: model, playback và AI là các luồng riêng

### PC/Web playback

Lazy PC, khoảng 1.322.849, dựng UI content từ `message.href`; parse `params`
để lấy `duration`, `m4a`, `waveformSamples`. Như vậy player có nguồn chính,
định dạng thay thế và metadata thật; không thể coi mọi voice chỉ là một URL MP3.

Đã đọc phần resource loader (80.600–83.050): kiểm tra trạng thái/expiry,
download vào local file, tạo blob/local URL và probe thời lượng. Hàm
`getDirectURL` (82.795) còn có đường chuyển đổi voice domain `/mp3?...`;
sự tồn tại hàm không chứng minh mọi tin đều đi qua đường đó.

Phần player (khoảng 1.487.400) đăng ký player theo key, xử lý `canplay`,
`loadedmetadata`, `timeupdate`, `ended`, error, và có `seekTo`/khôi phục progress.
Đã thấy cả UI mới có seek và UI cũ stop/replay, nên không gộp chúng thành một
control duy nhất khi đánh giá parity.

Fork dùng Media Foundation/CPAL để giải mã/phát inline. Đó là implementation
native riêng. Search Rust hiện chỉ thấy `m4a` trong danh sách extension chung,
chưa thấy xử lý tường minh `params.m4a`/`waveformSamples` hoặc conversion URL.
Không suy từ fixture WAV đã qua sang hỗ trợ mọi nguồn AMR/AAC của tài khoản.

### PC/Web AI voice

- Lazy PC `b7.requestConvert` (2.091.549): POST session-AES `{url}` tới
  service `ai_features`, path `/api/aifeatures/voice2text`, command 13394.
- Handler push (2.092.997): nhận `act=convert_finish`, kiểm tra `req_id` và
  numeric `err`; module `v9mf` map `has_more` thành trạng thái truncated.
- Web lazy `web-startup` có cùng handler (1.914.305).

`has_more` ở projection này là **isTruncated**; riêng tên trường không chứng
minh server sẽ tiếp tục streaming nhiều đoạn. Fork phải giữ đúng ý nghĩa này.
Request hiện tại của fork đã khớp subset `{url}` và dynamic service ở mức mã;
chưa có bằng chứng entitlement/server acceptance trên tài khoản thật.

### Android AI: đã đọc lệnh tạo packet

`classes5.dex`, `Luls;->a(Lxls;)V`:

1. instruction 30–33 dựng JSON có `url` và long `fileId`;
2. 52–59 đặt packet field `g=760`, timeout 30.000 ms, gán binary params rồi
   đưa qua network request wrapper;
3. callback `Luqn;->d(Object)` đọc `data.cliFileId`, so với ID của request,
   sau đó lấy `text` và `asr` (instruction 19–53).

Đây là luồng socket Android, không phải schema cho endpoint HTTP của PC.
`Luls;->e` có gate định dạng AAC và lỗi `FORMAT_NOT_SUPPORTED`; một nhánh khác
kiểm tra local file/network rồi đưa qua upload service trước khi chuyển đổi.
Chưa giải mã toàn bộ packet header/transport native và chưa chạy app Android.

Đã thấy code ghi `dictation_record_tracking` với `client_id`, `owner_id`,
`sender_id`, `asr`, `sent_client_id`; đây là tracking dictation, không tự suy
thành cùng cache cho mọi transcript của voice incoming.

### Outgoing PC

Đã đọc `static sendVoice` (15.098.602): dùng file service, DM/group paths riêng,
session-AES chứa `toid` hoặc `grid` cùng `clientId`, query type 2/11 và multipart
field `fileContent`, timeout 60 giây. Không chỉ tìm route trong index.
Chưa truy hết recorder → encoder để chứng nhận format của file đầu vào; tên
`voicemp3` và sự có mặt `libopus.so` trong APK không đủ chứng minh codec gửi.

## 5. Popup Phân loại

PC filter mở `ModalManagerV2` với identity `MANAGE_LABEL`. Trong component
quản lý, `_cancel` (11.379.124) có thể mở confirm khi đang sửa; khi được phép
đóng thì gọi `closeModal`, kèm stopPropagation/preventDefault. Component còn
có sub-view thêm hội thoại, sắp xếp nhãn và chỉnh màu.

Điều này xác nhận modal có lifecycle/input ownership và trạng thái chỉnh sửa.
Không chuyển z-index DOM của Electron thành `egui::Order` theo kiểu một-một.
Bản sửa Foreground và test click X của fork giải quyết lỗi input cụ thể trong
fork; chưa chứng nhận parity toàn bộ workflow Phân loại của Zalo.

## 6. ZaloData đã kiểm tra đến đâu

- Đọc tên khóa của `config.json`, `database-config.json`, `Local State`,
  `Preferences`, `mds-config.json`, `migrate-config.json`. Có nhóm `db-enc`,
  trạng thái migration, window geometry và `os_crypt.encrypted_key`; không đọc
  hoặc xuất giá trị key.
- Inventory đệ quy `Database` tại thời điểm đọc có **1.289 file `.db`**.
  Nhánh theo account có Core/Message shards, Media, Meta, MsgInfo, Reaction,
  Preferences, Res, Sync, E2ee. Đã che định danh account trong output.
- DB theo account lấy mẫu không có header SQLite plaintext. Điều này phù hợp
  cấu hình `db-enc`, nhưng chưa phân tích format để khẳng định thuật toán mã hóa.
- Một số DB chung đọc schema bằng SQLite read-only/immutable:
  `Storage.db` có `info-cache`, `friend-blocked`, `retry-cache`, `db-stchecksum`;
  `SecureLocalstorage.db` có `async-store`; `Trust.db` có các bảng trust/keyring.
  Không đọc nội dung profile/keyring hoặc message rows. Immutable bỏ qua WAL,
  nên không xem đây là snapshot dữ liệu mới nhất của app đang chạy.
- Scan marker giới hạn file dưới 32 MiB: 5 file Local Storage/leveldb,
  1 resp_cache và 6 Session Storage; không thấy các marker `zcinput`,
  `phonebook_key`, `alias_key`, `inputMaxLines` đã chọn. Tìm tên key trong các
  bảng key/value chung cũng không thấy marker đã chọn. **Không tìm thấy không
  chứng minh không tồn tại:** dữ liệu có thể bị mã hóa, nén, ở WAL hoặc nơi khác.

## 7. Các kết luận cần điều chỉnh cho fork

| Mục | Kết luận sau đọc trực tiếp | Việc còn thiếu |
| :--- | :--- | :--- |
| Popup | Lỗi lớp input của fork đã có bản sửa/test; PC có lifecycle confirm phức tạp hơn | Thử native và đối chiếu trạng thái đang chỉnh sửa |
| Alias/pbName | Thứ tự tên là một phần; nhánh alias v2 quyết định có dùng phonebook cũ hay không | Hydrate feature flags, chọn API đúng phiên, kiểm tra dữ liệu thật |
| Voice incoming | Inline player là hướng đúng; nguồn/codec/recovery và seek có nhiều nhánh | Đối chiếu nguồn m4a/AMR/AAC, nguồn expired/local, thiết bị phát và AI thật |
| Composer | Plain 5 dòng và expanded 1/2 là policy fork hiện tại; bản gốc có legacy/column và tỷ lệ 2/3 theo column | Phân biệt rollout/editor, đo cùng viewport/DPI; không tự đổi số theo default |

Chỉ đọc/viết tài liệu nghiên cứu trong lượt này. Đã kiểm tra hash, method và
anchor; không chạy test Rust cho thay đổi tài liệu, không tuyên bố đã sửa các
khoảng trống mới tìm thấy, không đổi trạng thái nghiệm thu thiết bị.
