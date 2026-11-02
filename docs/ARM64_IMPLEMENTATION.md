# Audit Windows 11 ARM64 — 2026-11-01

Phạm vi: mã nguồn Rust, worker/session/network, lưu trữ, GUI, audio/video, dependency native, workflow, bộ cài và tài liệu phát hành. Không đăng nhập Zalo, gửi tin hoặc gọi liên hệ thật trong quá trình audit.

## Đối chiếu upstream

Đã tích hợp upstream `main` tại `541ecb52cc43f671af41efc0c74c3c5502e93292` (dòng **v0.0.3**, đã nhập thêm 7 commit sau snapshot aad565c). Phần runtime Windows được giữ khi có triển khai chồng lấn: DPAPI, dữ liệu từng tài khoản, DX12, Media Foundation và tray. Các thay đổi hiệu năng, giờ địa phương, font và tên binary v0.0.3 đã được đưa vào. Không nhập self-updater đa nền tảng trỏ tới artifact upstream; xem [ADR-0001](adr/0001-windows-arm64-runtime.md) để biết phạm vi khác biệt. `origin/main` vẫn ở `3f28d2f`; các bản sửa nằm trên PR #2, chưa đồng nghĩa đã phát hành.

Những sửa đổi mới nằm trên nhánh `fix/windows-arm64-product`; không đồng nghĩa đã có trong `main` hoặc bản Release đang tải. Các PR upstream đang mở không được coi là code đã merge.

Đã đối chiếu các issue [#3](https://github.com/ctdoteam1110/ctdozlite/issues/3), [#5](https://github.com/ctdoteam1110/ctdozlite/issues/5), [#8](https://github.com/ctdoteam1110/ctdozlite/issues/8), [#10](https://github.com/ctdoteam1110/ctdozlite/issues/10), [#19](https://github.com/ctdoteam1110/ctdozlite/issues/19), [#21](https://github.com/ctdoteam1110/ctdozlite/issues/21), [#22](https://github.com/ctdoteam1110/ctdozlite/issues/22), [#23](https://github.com/ctdoteam1110/ctdozlite/issues/23), [#24](https://github.com/ctdoteam1110/ctdozlite/issues/24), [#27](https://github.com/ctdoteam1110/ctdozlite/issues/27), [#31](https://github.com/ctdoteam1110/ctdozlite/issues/31), [#32](https://github.com/ctdoteam1110/ctdozlite/issues/32) và PR #29/#30 đang mở.

## Các lỗi người dùng báo

| Vấn đề | Nguyên nhân/hướng sửa trong mã | Giới hạn kiểm chứng |
|---|---|---|
| Màn hình đen sau cài | Bật rõ feature biên dịch `wgpu/dx12` (trước đây chỉ chọn DX12 lúc runtime nhưng backend không có trong binary); DX12 làm mặc định; CRT tĩnh; kiểm tra import DLL; GUI smoke chụp framebuffer thật, từ chối ảnh trống. DLL Media Foundation delay-load và kiểm tra thiếu Media Feature Pack. | Cần thử GPU/driver của máy đã báo lỗi. |
| Không kéo/resize cửa sổ | Bỏ khung và hit-test tự vẽ, dùng caption/resizing của Windows. | Cần thử chuột/cảm ứng, Snap và nhiều màn hình. |
| Settings vỡ | Sửa phép tính lề bị cộng thừa 48px, bỏ kích thước tối thiểu vượt màn hình; tab dạng dropdown khi hẹp, nội dung cuộn, card co theo chiều rộng. | CI kiểm tra ảnh và biên cửa sổ ở 480×360, 1020×700/150%, 1280×900/200%. |
| Không tắt được app | X/Alt+F4 ẩn xuống tray thật; cả click trái/phải có Mở Apps và Thoát Apps; không join luồng driver media trên UI thread. Camera có watcher Shutdown để ngắt ReadSample khi dừng. | Test GUI kiểm tra HWND ẩn/mở lại, thoát từ trạng thái ẩn trong 45 giây; cần thử driver thật khi đang gọi. |
| Bấm đồng bộ không thấy gì | Tách cập nhật gần đây với tải lịch sử nhóm/Cloud; có phân trang, checkpoint SQLite, dừng/tiếp tục và báo lỗi. Không đổi lỗi danh bạ thành danh sách rỗng/thành công. | Chưa xác minh tải toàn bộ lịch sử chat cá nhân hoặc điện thoại. Cần test API bằng tài khoản thật. |
| Tất cả user đều online | Xóa chấm xanh/trạng thái cố định của liên hệ; trạng thái không có nguồn presence được để chưa biết. | Chưa có API presence thực. |
| Gọi đi chưa OK | Bỏ khóa test mặc định; sửa mất `typeRequest=3` khi khởi tạo video (trước đây fallback luôn gửi loại thoại 1); giữ wire shape riêng cho invite có metadata. | Chưa kết luận tương tác cuộc gọi đi/đến end-to-end. Cold-start invite chưa có relay metadata đầy đủ vẫn là đường tương thích cần kiểm tra. |
| Không dùng được camera | Thay FFmpeg/DirectShow bằng Media Foundation: enumerate, capture NV12, H.264 baseline encode/decode, preview native; báo lỗi quyền/thiết bị và cho bật lại. | Codec round-trip đã chạy native. Không có camera/microphone và tài khoản thật trên runner để xác nhận capture/call. |
| Nhóm chỉ có ID | getmg yêu cầu version 0 thay vì báo đã có version mới nhất; phân lô 50, xử lý JSON data dạng chuỗi; giữ tên cache khi lỗi và không ghi tên rỗng/placeholder đè tên thật trong DB. | Cần xác nhận metadata của tài khoản thực. |

## Các phát hiện bổ sung đã xử lý

- **Định danh hội thoại:** không gộp/migrate recipient ID theo tên hoặc avatar. Hai liên hệ trùng tên/ảnh phải giữ lịch sử và đích gửi/gọi riêng. Loại bỏ migration SQLite theo tên và thao tác xóa message xuyên hội thoại; giữ lựa chọn chat theo ID khi refresh. Xóa một tin có ID không được xóa các tin khác cùng timestamp. Hai tin thật cùng nội dung gửi trong 5 giây không bị gộp mất; heuristic chỉ đối soát placeholder gửi đi với echo server.
- **SQLite:** khóa tin là `(thread_id,msg_id)`, chuyển schema cũ bằng transaction; checkpoint lịch sử ghi sau message; tải trang cũ không làm lùi timestamp hoặc xóa tên nhóm. Không tự sửa dữ liệu đã bị bản cũ trộn vì thiếu bằng chứng sở hữu.
- **Phiên:** phân biệt Valid/Expired/Unreachable; chỉ mã server 600/-102 làm hết phiên, không dùng từ khóa zpw_sek/HTTP 404 để logout. Offline giữ credential/history. Explicit logout vẫn xóa phiên cục bộ.
- **Endpoint:** REST verified ưu tiên hơn service map; host discovery hợp lệ giữ làm fallback trong bộ nhớ, không lưu thành ưu tiên lâu dài. GET có thể đổi host khi connect/timeout/404; POST chỉ retry khi có HTTP 404 chắc chắn, tránh gửi trùng mutation sau timeout. Long-poll và WebSocket vẫn cập nhật động.
- **Build/test:** binary dùng library chung, bỏ việc khai báo lại toàn bộ module và chạy trùng cùng một bộ unit test ở lib/bin.
- **Giới hạn tài nguyên:** response/decompression/image có giới hạn; WebSocket giới hạn frame/message và queue có backpressure; debug preview tối đa 16 KiB/trường; enumerate thiết bị chạy nền, camera giữ COM MTA riêng để không xung đột STA của CPAL; tái dùng texture video thay vì tạo GPU texture mỗi frame.
- **Media:** bỏ frame video giả và trộn keyframe cũ với P-frame mới; giữ thứ tự frame. Lỗi decoder reset và yêu cầu keyframe lại. Các đường thoát sớm của audio dừng task camera/decoder. Không dùng tiến trình FFmpeg/ZaloCall ngoài app.
- **Windows:** file/folder picker Win32, shell API thay helper process, font Windows/CJK fallback, theme hệ thống lấy từ egui/winit.
- **Lưu trữ:** data root theo user, DPAPI, remember-login, account namespace, atomic replacement và khóa instance kế thừa từ bản sửa trước; không tự nhập cache/session vô chủ.

## Dependency audit

Đã truy vấn OSV trên 313 package registry thuộc graph normal/build của target Windows ARM64, không chỉ đọc tên crate trong manifest. Xem [kết quả máy đọc](DEPENDENCY_AUDIT.json).

| Phát hiện | Xử lý |
|---|---|
| Tokio 1.54.0 — RUSTSEC-2026-0384, race/use-after-free trong Interval | Lockfile cập nhật 1.54.1. Code WebSocket có sử dụng Interval nên lỗi có liên quan thực tế. |
| lru 0.12.5 — RUSTSEC-2026-0002 và RUSTSEC-2026-0253 | Bỏ implementation cũ; shim local không có unsafe re-export upstream lru 0.18.2 đã sửa, giữ tương thích ratatui 0.29. Xem vendor/lru-compat/README.md. |
| audiopus_sys, paste, ttf-parser — thông báo unmaintained | Còn trong dependency graph. Đây là rủi ro bảo trì, không phải bằng chứng đã có khai thác. Cần kế hoạch thay bindings/GUI/TUI; không che bằng ignore advisory. |

MD5/AES-CBC/zero-IV ở lớp giao thức là hành vi tương thích của Zalo, không phải bảo đảm mã hóa hiện đại hay end-to-end do fork tự thiết kế. Session trên đĩa dùng DPAPI; database/cache chưa mã hóa. Không phát hành log phiên hoặc dữ liệu chat.

## Dọn phạm vi sản phẩm

Đã bỏ workflow đa nền tảng, packaging khác Windows ARM64, metadata Debian, hướng dẫn build/release nền tảng khác và các binary probe/reverse-engineering không dùng trong sản phẩm. Bỏ bridge Named Pipe và mã tìm ZaloCall.exe cũ. Giữ giấy phép, tài liệu bằng chứng giao thức và tool đọc log cần cho bảo trì.

Release binary bị chặn compile ngoài Windows ARM64; một số implementation cho test host được giữ để kiểm thử protocol/storage debug. Pipeline duy nhất tạo installer và portable ARM64; job upload Release có quyền ghi riêng, build dùng quyền đọc. Không sửa lịch sử Git hoặc xóa release cũ của người dùng.

## Kiểm kê Settings

Rà soát nơi đọc giá trị ngoài panel Settings cho thấy nhiều tùy chọn upstream mới chỉ lưu JSON. Đây là phần chưa hoàn thiện của thiết kế, không được tính là tính năng đã có.

| Tùy chọn | Hành vi hiện có |
|---|---|
| Ghi nhớ đăng nhập, theme, chọn mic/camera | Đã nối với lưu phiên, giao diện và media. Theme “hệ thống” đọc theme của Windows qua winit/egui. |
| Thông báo | Cập nhật tiêu đề/unread và yêu cầu Windows chú ý tới cửa sổ; chưa có toast Windows hoặc âm báo theo lựa chọn. |
| Thư mục tải | Chọn, lưu và mở thư mục; link tải vẫn mở trình duyệt, nên chưa điều khiển nơi trình duyệt lưu file. |
| Gợi ý sticker khi gõ, tìm sticker theo từ khóa | Có nơi đọc giá trị và áp dụng trong UI sticker. |
| Hiện trạng thái online | Chỉ là dấu phiên cục bộ trong dock; không thay privacy/presence trên server. |
| Danh bạ toàn bộ, đổi ngôn ngữ, autostart, avatar làm wallpaper | Chưa có hành vi runtime đầy đủ. Autostart đang bị vô hiệu hóa trong UI. |
| Sinh nhật/đã xem/ai được nhắn-gọi/tìm qua số điện thoại/nguồn kết bạn | Chưa tích hợp server; đã gỡ các nút privacy giả và hướng dẫn dùng app chính thức. |
| Tin nhanh, PIN ẩn chat, chia priority, chế độ dán Excel, typing | Giá trị kế thừa được lưu nhưng chưa thực thi tương ứng đầy đủ. |
| Gợi ý mention và tìm GIF | Chưa có pipeline runtime tương ứng với các công tắc. |

Không nên phát hành như một bản “đầy đủ mọi tính năng thiết kế” khi các mục này và kiểm thử cuộc gọi thật chưa được hoàn tất.

## Kiểm chứng và những mục chưa hoàn tất

Test tự động dùng dữ liệu giả, gồm session/offline/expiry, metadata nhóm, identity isolation, migration/rollback, cursor, bounded payload, packetization, DPAPI và native codec. Native CI kiểm tra PE, DLL, chạy exe, thử framebuffer DX12 và tạo bộ cài. Kết quả và ảnh của **đúng commit** trong Actions là bằng chứng build; không dùng kết quả của commit trước để chứng nhận commit sau.

Cần test thiết bị/tài khoản thật trước khi gọi bản này “đầy đủ tính năng”:

1. Cài sạch và nâng cấp trên máy Windows 11 ARM64 đã bị màn hình đen; kéo/resize/Snap/DPI/đóng trong khi đang gọi.
2. Login, mất mạng rồi phục hồi, hết phiên thật, logout/chuyển account và giữ lịch sử.
3. Metadata nhóm, checkpoint tải lịch sử, duplicate/gián đoạn/rate-limit trên tài khoản được phép.
4. Gọi đi/đến audio/video với client Zalo chính thức, NAT/mạng khác nhau, mic/camera/Bluetooth, chuyển thiết bị, rút camera, sleep/resume.

Chưa hỗ trợ gọi nhóm, tải lại toàn bộ lịch sử 1-1 từ điện thoại, thay quyền riêng tư server, cập nhật ứng dụng tự động hoặc autostart. Một số tùy chọn UI kế thừa (ngôn ngữ khác, PIN ẩn chat, gợi ý typing/GIF/mention tùy mục) chưa có tác động đầy đủ; không coi việc lưu một giá trị setting là đã triển khai tính năng. Bộ cài chưa ký số và chưa có kiểm thử physical hardware.

## Ưu tiên bổ sung theo thiết bị đích

Huawei MateBook E Go Gen 3 / Snapdragon 8cx Gen 3. Đồng bộ tự bắt đầu sau đăng nhập và khi khôi phục phiên; nhận tin/persist hoạt động trong worker khi cửa sổ ẩn. CM dùng lastMsgId, hasMore, isOld và isFiltered của server; không dùng msgId làm cursor. Server có thể không cung cấp lịch sử trước thời điểm đăng nhập. Chat cá nhân cũ từ điện thoại vẫn chưa có API được kiểm chứng.

## Đo hiệu năng gộp lịch sử

Dữ liệu giả: 1.000 tin đang có + 1.000 tin đến, trùng 500 ID, nội dung lặp; 20 lượt trên Linux x86_64 trong cloud, profile debug. Trước tối ưu index: min 12,799 ms, trung bình 13,067 ms, p50 13,035 ms, p95 13,262 ms, p99 14,198 ms. Sau: min 2,183 ms, trung bình 2,344 ms, p50 2,232 ms, p95 2,586 ms, p99 3,559 ms. Chạy lại bằng `cargo run --locked --example history_bench`; đây không phải số đo trên Snapdragon 8cx Gen 3.

Bổ sung từ upstream sau `1345f5d`: các module nền tảng message/reaction/unread và chuẩn hóa timestamp đã được đưa vào cùng kiểm thử. Chưa coi các module này là bằng chứng trạng thái đọc/reaction đã đồng bộ đầy đủ từ mạng qua SQLite và UI. Cơ chế ghi phiên nguyên tử của upstream đã có triển khai tương đương trên DPAPI của fork, nên không thay bằng session JSON.

## Hoàn thiện tiếp sau 81a90d6

- Đã nhập upstream tới `541ecb5`, giữ tài liệu và pipeline chỉ Windows ARM64. Module updater upstream chỉ là nền tảng có kiểm thử, chưa có cập nhật ứng dụng tự động.
- RAM: hội thoại nền chỉ giữ 20 tin gần đây, hội thoại đang mở giữ cửa sổ lịch sử có giới hạn; toàn bộ tin vẫn ở SQLite. Tải lịch sử không tạo unread/thông báo giả. Worker không ghi lại mọi hội thoại từ UI sau mỗi trang.
- Đo dữ liệu giả 100 trang × 50 tin, 20 lượt debug Linux x86_64: trung bình trước **456,545 ms**, sau **17,689 ms**; resident message của hội thoại nền giảm 5.000 → 20. Chạy test bị ignore `history_residency_benchmark` riêng để đo lại. Không phải số đo RAM/RSS hoặc benchmark Snapdragon.
- Cache texture có ngân sách 128 MiB, preview có giới hạn kích thước; job ảnh giữ permit đến khi UI nhận để giới hạn cả hàng đợi thành công. Cache riêng tài khoản có giới hạn đĩa 512 MiB; cache lỗi thử tải lại, lỗi ghi cache không chặn ảnh hợp lệ. Preview video chỉ giữ một frame mỗi hướng/generation và bỏ công việc hiển thị khi cửa sổ ẩn.
- Camera tự ưu tiên RGB hơn IR/Hello, thương lượng NV12 không ép driver phải xuất 15 fps. Chuẩn hóa stride, scale/letterbox về 320×240; nhịp gửi vẫn 15 fps. Đọc buffer 2D loại bỏ padding phần cứng, camera treo có timeout. Chưa chứng nhận driver MateBook.
- Audio giải mã theo độ dài Opus thực (10/20/40/60 ms), giới hạn backlog loa; resampling giữ pha giữa callback và hỗ trợ nguồn 8 kHz Bluetooth. Lỗi mic/loa được báo riêng thay vì dừng toàn bộ video.
- SQLite lưu metadata trạng thái gửi/reaction/wire ID; recall là tombstone không bị trang cũ phục hồi nội dung. Tin mạng không xóa pin/star cục bộ; UI hiển thị trạng thái giao nhận theo dữ liệu hiện có. Chưa đồng nghĩa read receipt/reaction mạng đã hoàn thiện toàn bộ.
- Double-click reply đã đọc và áp dụng setting. Credential chỉ gắn cho HTTPS host Zalo hợp lệ; cookie Secure. Diagnostic mặc định che dữ liệu nhạy cảm trước khi ghi. `--dump-raw` vẫn là chế độ opt-in nhạy cảm.
- Kiểm thử local: **164 unit pass, 2 ignore** (benchmark thủ công và live-call), **30 integration pass**; fmt và clippy strict đạt. Kiểm tra biên dịch riêng module video cùng test cho `aarch64-pc-windows-msvc` đạt; CI native phải xác nhận lại toàn bộ trước khi dùng artifact mới.

Quyết định và giới hạn: [ADR-0002](adr/0002-bounded-media-and-durable-message-state.md). Các giới hạn cuộc gọi thật, lịch sử 1-1, updater và Settings liệt kê ở trên vẫn còn hiệu lực.
