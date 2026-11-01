# Audit Windows 11 ARM64 — 2026-11-01

Phạm vi: mã nguồn Rust, worker/session/network, lưu trữ, GUI, audio/video, dependency native, workflow, bộ cài và tài liệu phát hành. Không đăng nhập Zalo, gửi tin hoặc gọi liên hệ thật trong quá trình audit.

## Đối chiếu upstream

Upstream mặc định là `main`, commit `73849bde5e5ffff21537e92886891db510e9c857`, cũng là tag mới nhất **v0.0.2**, phát hành 2026-10-31. `origin/main` của fork ở `3f28d2f99413d84e31d979cb914be3cc7945fb15` chứa toàn bộ upstream và thêm hai commit ARM64 (`git rev-list --left-right --count upstream/main...origin/main`: `0 2`).

Những sửa đổi mới nằm trên nhánh `fix/windows-arm64-product`; không đồng nghĩa đã có trong `main` hoặc bản Release đang tải. Các PR upstream đang mở không được coi là code đã merge.

Đã đối chiếu các issue [#3](https://github.com/ctdoteam1110/ctdozlite/issues/3), [#5](https://github.com/ctdoteam1110/ctdozlite/issues/5), [#8](https://github.com/ctdoteam1110/ctdozlite/issues/8), [#10](https://github.com/ctdoteam1110/ctdozlite/issues/10), [#19](https://github.com/ctdoteam1110/ctdozlite/issues/19), [#21](https://github.com/ctdoteam1110/ctdozlite/issues/21), [#22](https://github.com/ctdoteam1110/ctdozlite/issues/22), [#23](https://github.com/ctdoteam1110/ctdozlite/issues/23), [#24](https://github.com/ctdoteam1110/ctdozlite/issues/24), [#27](https://github.com/ctdoteam1110/ctdozlite/issues/27), [#31](https://github.com/ctdoteam1110/ctdozlite/issues/31), [#32](https://github.com/ctdoteam1110/ctdozlite/issues/32) và PR #29/#30 đang mở.

## Các lỗi người dùng báo

| Vấn đề | Nguyên nhân/hướng sửa trong mã | Giới hạn kiểm chứng |
|---|---|---|
| Màn hình đen sau cài | DX12 thay OpenGL làm mặc định; CRT tĩnh; kiểm tra import DLL; GUI smoke chụp framebuffer thật, từ chối ảnh trống. DLL Media Foundation delay-load và kiểm tra thiếu Media Feature Pack. | Cần thử GPU/driver của máy đã báo lỗi. |
| Không kéo/resize cửa sổ | Bỏ khung và hit-test tự vẽ, dùng caption/resizing của Windows. | Cần thử chuột/cảm ứng, Snap và nhiều màn hình. |
| Settings vỡ | Sửa phép tính lề bị cộng thừa 48px, bỏ kích thước tối thiểu vượt màn hình; tab dạng dropdown khi hẹp, nội dung cuộn, card co theo chiều rộng. | CI kiểm tra ảnh và biên cửa sổ ở 480×360, 1020×700/150%, 1280×900/200%. |
| Không tắt được app | Bỏ CancelClose + Minimized ở nút X; không join luồng driver media trên UI thread. Camera có watcher Shutdown để ngắt ReadSample khi dừng. | Test GUI yêu cầu tiến trình tự thoát trong 45 giây; cần thử driver thật khi đang gọi. |
| Bấm đồng bộ không thấy gì | Tách cập nhật gần đây với tải lịch sử nhóm/Cloud; có phân trang, checkpoint SQLite, dừng/tiếp tục và báo lỗi. Không đổi lỗi danh bạ thành danh sách rỗng/thành công. | Chưa xác minh tải toàn bộ lịch sử chat cá nhân hoặc điện thoại. Cần test API bằng tài khoản thật. |
| Tất cả user đều online | Xóa chấm xanh/trạng thái cố định của liên hệ; trạng thái không có nguồn presence được để chưa biết. | Chưa có API presence thực. |
| Gọi đi chưa OK | Bỏ khóa test mặc định; sửa mất `typeRequest=3` khi khởi tạo video (trước đây fallback luôn gửi loại thoại 1); giữ wire shape riêng cho invite có metadata. | Chưa kết luận tương tác cuộc gọi đi/đến end-to-end. Cold-start invite chưa có relay metadata đầy đủ vẫn là đường tương thích cần kiểm tra. |
| Không dùng được camera | Thay FFmpeg/DirectShow bằng Media Foundation: enumerate, capture NV12, H.264 baseline encode/decode, preview native; báo lỗi quyền/thiết bị và cho bật lại. | Codec round-trip đã chạy native. Không có camera/microphone và tài khoản thật trên runner để xác nhận capture/call. |
| Nhóm chỉ có ID | getmg yêu cầu version 0 thay vì báo đã có version mới nhất; phân lô 50, xử lý JSON data dạng chuỗi; giữ tên cache khi lỗi và không ghi tên rỗng/placeholder đè tên thật trong DB. | Cần xác nhận metadata của tài khoản thực. |

## Các phát hiện bổ sung đã xử lý

- **Định danh hội thoại:** không gộp/migrate recipient ID theo tên hoặc avatar. Hai liên hệ trùng tên/ảnh phải giữ lịch sử và đích gửi/gọi riêng. Loại bỏ migration SQLite theo tên và thao tác xóa message xuyên hội thoại.
- **SQLite:** khóa tin là `(thread_id,msg_id)`, chuyển schema cũ bằng transaction; checkpoint lịch sử ghi sau message; tải trang cũ không làm lùi timestamp hoặc xóa tên nhóm. Không tự sửa dữ liệu đã bị bản cũ trộn vì thiếu bằng chứng sở hữu.
- **Phiên:** phân biệt Valid/Expired/Unreachable; chỉ mã server 600/-102 làm hết phiên, không dùng từ khóa zpw_sek/HTTP 404 để logout. Offline giữ credential/history. Explicit logout vẫn xóa phiên cục bộ.
- **Endpoint:** REST verified ưu tiên hơn service map; host discovery hợp lệ giữ làm fallback trong bộ nhớ, không lưu thành ưu tiên lâu dài. GET có thể đổi host khi connect/timeout/404; POST chỉ retry khi có HTTP 404 chắc chắn, tránh gửi trùng mutation sau timeout. Long-poll và WebSocket vẫn cập nhật động.
- **Giới hạn tài nguyên:** response/decompression/image có giới hạn; WebSocket giới hạn frame/message và queue có backpressure; debug preview tối đa 16 KiB/trường; enumerate thiết bị chạy nền, camera giữ COM MTA riêng để không xung đột STA của CPAL; tái dùng texture video thay vì tạo GPU texture mỗi frame.
- **Media:** bỏ frame video giả và trộn keyframe cũ với P-frame mới; giữ thứ tự frame. Lỗi decoder reset và yêu cầu keyframe lại. Các đường thoát sớm của audio dừng task camera/decoder. Không dùng tiến trình FFmpeg/ZaloCall ngoài app.
- **Windows:** file/folder picker Win32, shell API thay helper process, font Windows/CJK fallback, theme hệ thống lấy từ egui/winit.
- **Lưu trữ:** data root theo user, DPAPI, remember-login, account namespace, atomic replacement và khóa instance kế thừa từ bản sửa trước; không tự nhập cache/session vô chủ.

## Dependency audit

Đã truy vấn OSV trên 300 package registry thuộc graph normal/build của target Windows ARM64, không chỉ đọc tên crate trong manifest. Xem [kết quả máy đọc](DEPENDENCY_AUDIT.json).

| Phát hiện | Xử lý |
|---|---|
| Tokio 1.54.0 — RUSTSEC-2026-0384, race/use-after-free trong Interval | Lockfile cập nhật 1.54.1. Code WebSocket có sử dụng Interval nên lỗi có liên quan thực tế. |
| lru 0.12.5 — RUSTSEC-2026-0002 và RUSTSEC-2026-0253 | Bỏ implementation cũ; shim local không có unsafe re-export upstream lru 0.18.2 đã sửa, giữ tương thích ratatui 0.29. Xem vendor/lru-compat/README.md. |
| audiopus_sys, paste, ttf-parser — thông báo unmaintained | Còn trong dependency graph. Đây là rủi ro bảo trì, không phải bằng chứng đã có khai thác. Cần kế hoạch thay bindings/GUI/TUI; không che bằng ignore advisory. |

MD5/AES-CBC/zero-IV ở lớp giao thức là hành vi tương thích của Zalo, không phải bảo đảm mã hóa hiện đại hay end-to-end do fork tự thiết kế. Session trên đĩa dùng DPAPI; database/cache chưa mã hóa. Không phát hành log phiên hoặc dữ liệu chat.

## Dọn phạm vi sản phẩm

Đã bỏ workflow đa nền tảng, packaging khác Windows ARM64, metadata Debian, hướng dẫn build/release nền tảng khác và các binary probe/reverse-engineering không dùng trong sản phẩm. Bỏ bridge Named Pipe và mã tìm ZaloCall.exe cũ. Giữ giấy phép, tài liệu bằng chứng giao thức và tool đọc log cần cho bảo trì.

Release binary bị chặn compile ngoài Windows ARM64; một số implementation cho test host được giữ để kiểm thử protocol/storage debug. Pipeline duy nhất tạo installer và portable ARM64; job upload Release có quyền ghi riêng, build dùng quyền đọc. Không sửa lịch sử Git hoặc xóa release cũ của người dùng.

## Kiểm chứng và những mục chưa hoàn tất

Test tự động dùng dữ liệu giả, gồm session/offline/expiry, metadata nhóm, identity isolation, migration/rollback, cursor, bounded payload, packetization, DPAPI và native codec. Native CI kiểm tra PE, DLL, chạy exe, thử framebuffer DX12 và tạo bộ cài. Kết quả và ảnh của **đúng commit** trong Actions là bằng chứng build; không dùng kết quả của commit trước để chứng nhận commit sau.

Cần test thiết bị/tài khoản thật trước khi gọi bản này “đầy đủ tính năng”:

1. Cài sạch và nâng cấp trên máy Windows 11 ARM64 đã bị màn hình đen; kéo/resize/Snap/DPI/đóng trong khi đang gọi.
2. Login, mất mạng rồi phục hồi, hết phiên thật, logout/chuyển account và giữ lịch sử.
3. Metadata nhóm, checkpoint tải lịch sử, duplicate/gián đoạn/rate-limit trên tài khoản được phép.
4. Gọi đi/đến audio/video với client Zalo chính thức, NAT/mạng khác nhau, mic/camera/Bluetooth, chuyển thiết bị, rút camera, sleep/resume.

Chưa hỗ trợ gọi nhóm, tải lại toàn bộ lịch sử 1-1 từ điện thoại, thay quyền riêng tư server, cập nhật ứng dụng tự động hoặc autostart. Một số tùy chọn UI kế thừa (ngôn ngữ khác, PIN ẩn chat, gợi ý typing/GIF/mention tùy mục) chưa có tác động đầy đủ; không coi việc lưu một giá trị setting là đã triển khai tính năng. Bộ cài chưa ký số và chưa có kiểm thử physical hardware.
