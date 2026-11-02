# Rà soát commit upstream mới — 2026-11-02

Snapshot đọc: [`20e742c098ec546515701dcf1efe6b92fe9ea8ee`](https://github.com/ctdoteam1110/ctdozlite/commit/20e742c098ec546515701dcf1efe6b92fe9ea8ee). So với upstream đã nhập trong fork (`541ecb5`): 13 commit gồm 3 merge commit, 54 file thay đổi, 9.667 dòng thêm và 1.362 dòng xóa. Fork được đối chiếu tại `4199326`; phiên này chỉ rà soát và bổ sung tài liệu, chưa nhập các commit mới vào runtime.

Release upstream `v0.0.4` đã phát hành; hai artifact Windows là `portable-windows-x86_64.exe` và `setup-windows-x86_64.exe`. **Không có bản Windows ARM64 trong release đó.** Tên phiên bản và trạng thái issue đóng không chứng minh tương thích ARM64 hoặc hoàn tất kiểm thử thực tế.

## Danh sách commit

| Commit | Thay đổi thực tế | Hướng xử lý cho fork |
|---|---|---|
| `6f105b3` | Lịch sử nhóm/Cloud, phân trang và nút dừng | Fork đã có cơ chế tương ứng; giữ checkpoint server và DB từng tài khoản của fork. |
| `8e08f0e` | Đưa timer polling vào worker | Fork đã có polling nền khi ẩn tray; không nhập lại timer/global worker cũ. |
| `3c87e64` | Cập nhật issue #56 trên landing | Ngoài phạm vi sản phẩm Windows ARM64. |
| `32fd9a5` | Landing và ghi nhận đóng góp/issue RAM | Giữ attribution khi cần, không nhập landing hoặc coi nhãn issue là số đo. |
| `f35cefe` | Sửa thông số benchmark trong README nhiều ngôn ngữ | Không dùng số đo upstream làm kết quả của fork/Snapdragon. |
| `c774585` | Merge PR #46 (polling) | Không phải một implementation độc lập thứ hai. |
| `d991805` | Merge PR #45 (history) | Không phải một implementation độc lập thứ hai. |
| `3890e5e` | Menu hội thoại, theme/sticker JSON, import DB, scheduler, REST/MCP/auto-reply, failover | Cần chia thành từng thay đổi, nối account lifecycle/worker và sửa các vấn đề dưới đây. |
| `23f06b7` | v0.0.4, Windows resources, Social Feed, gọi nhóm/SFU và call card | Có phần hữu ích nhưng chưa thể nhập nguyên trạng: manifest amd64, Feed tổng hợp dữ liệu, media FFmpeg. |
| `5009566` | Sửa lipo macOS | Không nhập vào sản phẩm Windows ARM64. |
| `7cc51c9` | Thông báo/cài cập nhật GitHub, PE version/manifest, sửa UI/worker | Phải đổi nguồn release, chọn đúng kiến trúc, giữ DPAPI/DX12 và kiểm tra rollback Windows. |
| `ced1a42` | Merge các thay đổi còn lại | Tiêu đề “resolve remaining issues” không thay thế kiểm chứng từng tính năng. |
| `20e742c` | Đổi tài liệu/landing thành 0 issue mở | Chỉ là trạng thái tracker, không phải sửa runtime. |

## Phát hiện cần chặn khi tích hợp

### 1. Updater có thể chọn x64 cho Windows ARM64 — đã tái hiện

[`src/updater.rs:393`](https://github.com/ctdoteam1110/ctdozlite/blob/20e742c/src/updater.rs#L393): nhánh Windows chấm điểm `.exe` theo portable/setup nhưng bỏ qua `target_arch`. Gọi hàm với tên `ctdozlite-0.0.4-portable-windows-x86_64.exe`, OS `windows`, arch `aarch64` trả **110**. Parser release sau đó chọn đúng file x64 này cho target ARM64. Release thật hiện chỉ có hai file Windows x64, nên đây là rủi ro phù hợp với dữ liệu phát hành hiện tại.

Nguồn discovery còn trỏ `ctdoteam1110/ctdozlite`, không phải fork. Trước khi dùng cần giới hạn owner/repo, kiến trúc asset và PE Machine ARM64, xác minh integrity rồi xử lý thay thế/rollback Windows. Checksum không chứng minh binary có đúng kiến trúc. Cơ chế rollback portable Windows nằm ở adapter worker riêng, cần kiểm chứng đường launch-failure thực tế; không suy từ test installer core.

### 2. Manifest Windows mới ghi cứng amd64 — đọc mã xác nhận

[`build.rs:240`](https://github.com/ctdoteam1110/ctdozlite/blob/20e742c/build.rs#L240) đặt `processorArchitecture="amd64"`. Không phù hợp với app ARM64. Build script upstream cũng dùng cfg của host cho resource; fork đã chuyển sang đọc target OS. Có thể tiếp nhận version resource/asInvoker, nhưng phải phát sinh manifest phù hợp target và kiểm tra resource trong PE native. Chưa chạy binary upstream ARM64, vì vậy không khẳng định manifest này là nguyên nhân của mọi màn hình đen.

Không có kết quả quét Defender/SmartScreen trong lần rà soát này; việc thêm metadata hoặc bỏ strip không bảo đảm hết false positive.

### 3. HTTP API bảo vệ token tùy chọn, parser đọc một lần — đã tái hiện bằng fixture

[`src/api_mcp.rs:726`](https://github.com/ctdoteam1110/ctdozlite/blob/20e742c/src/api_mcp.rs#L726) chỉ bật kiểm tra token khi có `CTDOZLITE_API_TOKEN`; không cấu hình biến này thì server vẫn nhận yêu cầu. Server bind loopback và chỉ được bật qua tùy chọn/env, **không mặc định phơi ra Internet**, nhưng trả CORS `*`, có endpoint đọc lịch sử và gửi/hẹn giờ tin.

Vòng HTTP chỉ gọi `read` một lần với 64 KiB, không chờ đủ `Content-Length`; không có giới hạn số connection/task hoặc timeout đọc. Fixture dùng chính vòng HTTP upstream, thay handler bằng hàm vô hại: gửi header POST có `Content-Length: 123` nhưng chưa gửi body; server gọi handler ngay với body dài 0. Cũng xác nhận yêu cầu không token được nhận và response có CORS `*`. Không truy cập API tài khoản thật hoặc gửi tin.

Nếu tích hợp: server phải opt-in, token bắt buộc, kiểm tra Origin/Host phù hợp, parser HTTP đầy đủ với giới hạn body/deadline/concurrency. Gắn server với session hiện hành và dừng khi logout/chuyển account. Không suy rằng mọi trình duyệt đều truy cập được loopback vì còn chính sách local-network riêng.

### 4. Scheduler có trạng thái gửi kẹt và chưa tách account — đã tái hiện phần claim

[`src/scheduler.rs:367`](https://github.com/ctdoteam1110/ctdozlite/blob/20e742c/src/scheduler.rs#L367) đổi `pending` thành `sending` trong transaction, nhưng các lần poll sau chỉ chọn `pending`. Fixture mở/đóng connection như khởi động lại: job đã claim vẫn `sending` và không được claim tiếp. Nếu dừng trước gửi thì bị bỏ lại; nếu dừng sau server nhận nhưng trước ghi kết quả thì trạng thái giao hàng không chắc chắn. Cần cơ chế phục hồi có trạng thái không chắc chắn và đối soát, không tự retry mù để tránh gửi trùng.

Worker và UI còn truyền DB tương đối `zalo_history.db`; job không có account owner trong schema. Khi tích hợp phải dùng DB/account scope của fork, tránh gửi job của account cũ bằng session mới. Các thao tác SQLite của modal cần chuyển qua worker. Auto-reply trên delta poll chưa thấy dedup bền theo message ID; cần test replay trước khi bật.

### 5. Social Feed chưa phải Nhật ký đầy đủ từ server — đọc mã xác nhận

[`src/feed.rs:345`](https://github.com/ctdoteam1110/ctdozlite/blob/20e742c/src/feed.rs#L345) gán album cá nhân 12 like/2 comment; dòng 432 suy mốc thời gian từ vị trí; dòng 451–455 tính số like/comment từ UID. Timeline ghép cover/avatar/status và media chia sẻ, không phải mọi trường đều là dữ liệu Feed thực. Có packet builder nhưng riêng sự tồn tại của builder chưa chứng minh đã fetch/publish Feed end-to-end.

Không nhập các số hoặc mốc thời gian này thành dữ liệu thật. Muốn giữ màn hồ sơ/album cần ghi nhãn đúng dữ liệu có nguồn, bỏ chỉ số giả và xác nhận API/protocol bằng fixture. Không gọi tính năng này là đồng bộ Nhật ký đầy đủ.

### 6. Không thay camera native và lịch sử của fork bằng implementation upstream

Upstream [`src/media.rs:897`](https://github.com/ctdoteam1110/ctdozlite/blob/20e742c/src/media.rs#L897) vẫn capture qua FFmpeg/DirectShow và đòi executable ngoài app. Giữ Media Foundation ARM64 của fork.

[`src/history_sync.rs:18`](https://github.com/ctdoteam1110/ctdozlite/blob/20e742c/src/history_sync.rs#L18) dùng DB chung tương đối; `next_cursor` tìm ID tin cũ nhất thay vì checkpoint `lastMsgId` trong response. Fork đã có cursor/flags server, chống replay và cancel/resume. Upstream cũng nói rõ lịch sử 1-1 ngoài phạm vi; cập nhật này không giải quyết toàn bộ tin cũ từ điện thoại.

### 7. Không hạ dependency hoặc hồi quy bảo vệ lưu trữ

Lockfile upstream vẫn Tokio **1.54.0** và lru **0.12.5**, trong khi fork đã lên Tokio **1.54.1** và shim lru re-export **0.18.2** theo audit dependency hiện có. Không thay lockfile fork bằng upstream. Chưa chạy lại OSV trong phiên rà soát commit này; căn cứ advisory nằm trong `DEPENDENCY_AUDIT.json`.

Import lịch sử upstream đọc toàn bộ file vào RAM rồi mở DB nguồn bằng `MessageStore::open` có khả năng khởi tạo/migrate; nên đổi sang đọc nguồn read-only có giới hạn và kiểm tra account provenance. Giữ khóa tin `(thread_id,msg_id)`, recall tombstone và local pin/star của fork, không nhập schema message-ID-only.

### 8. Failover không phân biệt GET và mutation

Trong snapshot cuối, `should_failover_candidate` không retry HTTP 5xx như changelog mô tả, nhưng retry transport/timeout cho cả GET và POST qua cùng `request_with_candidates`. Với POST, timeout có thể xảy ra sau server đã xử lý nên gửi sang host tiếp có thể tạo mutation trùng. Giữ chính sách retry mutation hạn chế của fork. Phân biệt connect-before-send với timeout-after-send; cần fixture cho trường hợp server đã nhận rồi ngắt response.

## Phần đáng tích hợp tiếp

1. Version resource/asInvoker đúng ARM64; không quảng cáo đã khắc phục Defender khi chưa đo.
2. Menu ghim/tắt thông báo/ẩn và theme/sticker JSON, nối qua worker và account-scoped persistence; kiểm tra endpoint mutations bằng fixture.
3. Sửa chọn relay đã probe, nested `callSetting`, packet SFU/layer/subscription và quản lý thành viên rời nhóm. Tách thay đổi pure parser/packet có test trước, rồi mới nối lifecycle; giữ native media. Đây chủ yếu là cuộc gọi nhóm, chưa chứng minh sửa được mọi cold-start 1-1 invite.
4. Scheduler sau thiết kế account isolation, crash recovery/uncertain delivery và dedup.
5. Import lịch sử sau kiểm tra ownership, schema và giới hạn tài nguyên. REST/MCP/auto-reply, Feed và updater cần hardening như trên trước khi bật.

## Kiểm chứng đã thực hiện

- Fetch/đối chiếu commit graph, diff 54 file và nguồn release upstream. Không merge/cherry-pick các commit này vào runtime fork trong phiên này.
- Harness biệt lập dùng trực tiếp module updater/scheduler upstream: 5 unit test sẵn có và 2 test tái hiện lỗi. Một test HTTP dùng vòng server upstream với Session/handler giả, chỉ kết nối loopback. **Tổng 8 test đạt**; test đạt ở đây xác nhận quan sát lỗi, không có nghĩa lỗi đã sửa.
- Chưa chạy native Windows, camera/call thật hoặc quét AV cho upstream v0.0.4. Không dùng credential/chat/contact thật. Bộ 194 test của fork không được coi là kiểm chứng cho các tính năng upstream chưa nhập.

Kết luận áp dụng: nhập có chọn lọc và giữ các bảo vệ ARM64 hiện tại. Không merge toàn bộ v0.0.4 chỉ dựa vào tiêu đề “resolve all issues”.

## Kiểm tra tiếp sau triển khai đồng bộ điện thoại

Upstream HEAD kiểm tra trực tiếp: **56ea37409edf734011de22626a817eba97c60288** (v0.0.5); có **13 commit mới** sau snapshot `20e742c098ec546515701dcf1efe6b92fe9ea8ee`. Fork chưa đồng nhất toàn bộ với upstream này; không merge toàn bộ vì khác kiến trúc và phạm vi nền tảng.

- `9d86331` (#63): sửa trùng tên làm gộp nhầm contact/nhảy chat. Fork đã giữ định danh thread chính xác, không cho tên/avatar làm bằng chứng di chuyển lịch sử; giữ chính sách chặt hơn cả ngoại lệ Cloud của upstream.
- `c65e82e` / `f347d0e` (#61): chỉ gộp tin theo server/client identity, cấp client ID trước khi gửi, giữ thời gian server khi nhận acknowledgement. Đây là thay đổi rộng trên chat/GUI/worker/call cần port riêng cùng test, chưa nhận nguyên khối trong lần đồng bộ điện thoại. Với dữ liệu điện thoại mới, fork chặn đối chiếu pending theo text/time, dùng server ID hoặc fallback client ID có namespace sender; khi đối chiếu cache phải duy nhất và cùng chiều/sender. Không coi hai nội dung giống nhau là một tin.
- `7529696` / `99113eb` (#60): bấm quote để cuộn tới đúng tin **đã nạp**, không tải lại toàn bộ lịch sử. Hữu ích nhưng chưa port UI trong lần này; không đánh dấu đã có tính năng khi chưa nối rendering.
- `563ad44` / `069f594` (#59): quyền notification macOS; không nhận vào sản phẩm Windows ARM64.
- `df05579`: v0.0.5 và đọc changelog GitHub Releases. Không nhận bộ updater chưa phân biệt ARM64/x64 của upstream.
- Các commit còn lại cập nhật tài liệu/credit/.mailmap. Giữ credit upstream gốc trong README, không nhập landing page hoặc số liệu đóng góp không áp dụng cho fork.

Không có bằng chứng trong 13 commit mới này cho một luồng đồng bộ điện thoại của upstream có thể dùng thay triển khai ADR-0005. Trạng thái upstream có thể tiếp tục thay đổi sau thời điểm kiểm tra; SHA trên là mốc đã đọc.

## Tích hợp tiếp #61 và #60

Đã port có chọn lọc `c65e82e` (định danh/đối soát tin) và `7529696` (điều hướng trích dẫn đã nạp), giữ kiến trúc ARM64 của fork. Bổ sung đối soát SQLite trong transaction, kiểm thử hydrate/rollback và alias self của lịch sử điện thoại. Những dòng “chưa port” ở phần snapshot trên là trạng thái trước đợt tích hợp này. Phạm vi và giới hạn cuối cùng nằm trong [ADR-0006](adr/0006-exact-message-identity-and-quote-navigation.md); không có nghĩa đã merge toàn bộ v0.0.5.

## Rà soát và tiếp nhận v0.0.6

Upstream HEAD kiểm tra: **ed550f6** (v0.0.6), sau mốc `56ea374`. Commit iOS (`478f34c`…`8e16464`) đã bị upstream xóa ở `d5833c2`, không có gì để nhập.

- `9328cc9` #67 đồng bộ tắt thông báo: **đã port** (getmute/setmute/control `act_type: "mute"`), fail-closed khi đổi, lần đầu đẩy các hội thoại đã tắt cục bộ lên máy chủ.
- `9328cc9` #64 proxy và nhiều hồ sơ: **đã port** với khác biệt: proxy không lưu vào `session.json`, có hỗ trợ socks thật (bật feature `socks`) và WebSocket cũng đi qua proxy; tên hồ sơ sai bị từ chối thay vì tự lọc ký tự.
- `9328cc9` #68 đồng bộ điện thoại: **không nhập**; chỉ gửi tín hiệu rồi đọc lại tin gần đây, fork đã có luồng phê duyệt trên điện thoại (ADR-0005/0007/0010).
- `ed550f6` RTF: **đã port phần hiển thị tin nhận**, giữ nguyên nội dung chữ; không port tự chuyển Markdown khi gửi (upstream biến `2*3*4` thành "234" có chữ nghiêng).
- `ed550f6` trang Danh bạ, ô soạn 5 dòng: **đã port** bản rút gọn, bỏ hai tab lời mời luôn trống.
- `ed550f6` sticker "Mèo Ami Bụng Bự", giao diện hẹn giờ mới: **không nhập** (bản quyền ảnh chưa rõ; fork đã có bộ hẹn giờ).

Chi tiết quyết định: [ADR-0013](adr/0013-upstream-v006-selective-port.md).
