# Đối chiếu yêu cầu upstream — 03/11/2026

Mốc fork trước bổ sung: `e80d251`. Mốc upstream đã đọc trực tiếp: `ed550f6b5a69cbe154d68c28874104d7368d36bf` (HEAD `main` lúc kiểm tra). Đọc nội dung và thảo luận của cả 13 issue/PR được chỉ định, không suy từ nhãn Closed/Merged. Trạng thái “đã có” dưới đây nói về mã và fixture; không thay cho kiểm thử tài khoản/điện thoại thật.

## Commit ed550f6

RTF nhận, Danh bạ A–Z và composer năm dòng đã có từ `15344dd`; không nhập lại. Bổ sung trong đợt này:

- 15 GIF Ami, thumbnail nhúng, tab bộ chọn/mục phổ biến/gợi ý. Gửi và chuyển tiếp bằng attachment streaming với client ID gốc, lỗi giữ retry; category/ID cục bộ không gửi lên API sticker thật. Preview dùng PNG tĩnh như upstream.
- Hộp hẹn giờ riêng có backdrop, bốn mốc nhanh, giờ cụ thể, cảnh báo client-side; dùng scheduler bền vững hiện có, gắn recipient/owner lúc mở. Nút đồng hồ cạnh composer và menu nút Gửi; bảng Công cụ giữ quản lý lịch cũ.
- Các nút composer nằm trên hàng trên khi editor tăng chiều cao; đo cả dòng tự ngắt khi drawer làm vùng chat hẹp. Vùng rất hẹp/DPI cao chuyển tác vụ phụ vào menu nút Gửi. Giữ editor ID, Unicode, Enter/Shift+Enter và giới hạn năm dòng.
- Nhớ trạng thái mở/đóng bảng thông tin hội thoại, ghi thiết lập qua worker.

Giữ khác biệt đã chốt: không tự biến Markdown gửi đi thành RTF làm thay đổi chuỗi như `2*3*4`; không thêm hai tab lời mời không có dữ liệu; không thay nguồn updater/target ARM64/version của fork bằng metadata upstream. Quyết định: [ADR-0016](adr/0016-upstream-v006-ui-and-bundled-gifs.md).

## Bảng đối chiếu

| Yêu cầu | Trạng thái fork | Bằng chứng và giới hạn |
| --- | --- | --- |
| [#63 — trùng tên nhảy nhầm chat](https://github.com/ctdoteam1110/ctdozlite/issues/63) | Đã có xử lý định danh | `chat::threads_represent_same_contact`, hydrate/merge và phục hồi selection dùng thread ID, không gộp theo tên/avatar. Test distinct IDs cùng tên/avatar. Riêng avatar người gửi nhóm vẫn có lookup tên trong workspace; đây không phải bằng chứng nhảy recipient nhưng cần cải thiện để tránh lấy nhầm avatar khi trùng tên. |
| [#27 — host dự phòng](https://github.com/ctdoteam1110/ctdozlite/issues/27) | Đã có, retry bảo thủ hơn | `Session::send` có candidate và nhớ host thắng. Chuyển sau HTTP 404; GET còn chuyển khi connect/timeout. Dừng lỗi nghiệp vụ/auth. Không replay POST sau timeout vì máy chủ có thể đã xử lý. Không đánh dấu đã retry mọi HTTP 5xx; cả nhận xét đóng issue và hành vi nguồn phải được phân biệt. Test remembered host và credential destination. |
| [#68 — đồng bộ điện thoại](https://github.com/ctdoteam1110/ctdozlite/issues/68) | Có implementation riêng sâu hơn | `phone_sync/{transfer,assets,pipeline,codec,import,plan}`: phê duyệt điện thoại, codec ghim hash, giải mã container, mapping/quarantine, checkpoint, tải/nhập giới hạn. Upstream `sync_phone_messages` chủ yếu wake-up, tối đa 5 lượt polling và đọc lại recent/cloud/group; không có cùng đường giải mã container. Chưa nghiệm thu Android/iOS thật cho lần sửa. |
| [PR #61 — hợp nhất tin đúng định danh](https://github.com/ctdoteam1110/ctdozlite/pull/61) | Đã có | `chat::message_identity_tests`, send client ID qua fallback, `store::reconcile_send_identity`, transaction/hydrate/rollback. Có regression push trước acknowledgement và giữ chronology. Không so trùng bằng text/time. |
| [PR #60 — nhấn quote](https://github.com/ctdoteam1110/ctdozlite/pull/60) | Đã có | `message_navigation`, `QuoteJump`, mở rộng render window, scroll/highlight theo reference duy nhất; có egui regression. Đích chưa nạp báo rõ, không tự kéo toàn bộ lịch sử. |
| UniKey/Telex/VNI | Có biện pháp nền; chưa nghiệm thu phần mềm thật | Editor có `Id::new(("chat_composer_input",thread_id))`; popover floating, debounce 145/200 ms và tránh kích hoạt từ Telex chưa hoàn tất; giống các thay đổi upstream `4982171`/`a5d3fc4`. Chưa có bài Windows tương tác chạy UniKey; không coi Unicode/fixture là bằng chứng đã sửa mọi lỗi IME. |
| [#5 — CJK, ảnh, flicker](https://github.com/ctdoteam1110/ctdozlite/issues/5) | Có nhiều phần; chưa tương đương trọn bộ | Có font CJK fallback, ảnh inline/thumb trước, cửa sổ render 150 và tăng dần, sidebar bỏ vẽ row ngoài viewport, media/avatar có clip guard, panel sync cố định. Nhưng `gui::update` vẫn yêu cầu repaint mỗi 1.200 ms; chưa tương đương commit bỏ timer của upstream. CJK còn phụ thuộc font hệ thống phù hợp. Không khẳng định flicker đã hết trong nhóm thật. |
| Xóa tin nhắn | Đã có | Menu `DeleteForMe` → API delete với wire/server/client identity, chỉ xóa SQLite/UI khi server xác nhận; thu hồi là thao tác riêng cho tin tự gửi. Không coi delete-for-me là thu hồi mọi phía. |
| Xóa cuộc trò chuyện | Mới có xóa lịch sử cục bộ | `ClearThreadHistory` có xác nhận, xóa messages/checkpoint trong transaction, giữ thread identity/preference. Chưa có thao tác xóa toàn bộ conversation đồng bộ như Zalo PC; lịch sử có thể quay lại khi sync. |
| [#9 — thứ tự/nhóm/ảnh/import](https://github.com/ctdoteam1110/ctdozlite/issues/9) | Có phần lớn, import có phạm vi | Có sidebar chronology, group metadata/history, pin hội thoại, ảnh inline; `tools::import_history` nhận SQLite/JSON đúng schema có xác nhận account, nguồn read-only, transaction và giới hạn. Không tự dò/import mọi thư mục Zalo PC hoặc tự hiểu mọi DB mã hóa. Các container điện thoại có đường giải mã riêng, không đồng nghĩa importer nhận mọi bản backup PC. |
| [#24 — giao diện/file/nhóm/các thao tác](https://github.com/ctdoteam1110/ctdozlite/issues/24) | Một phần | Có nhóm, media card/save, responsive, delete-for-me, ẩn hội thoại và lịch gửi. Xóa conversation vẫn local-only; không có `ScheduledKind::Reminder`/nhắc hẹn riêng trong fork. Hẹn giờ gửi tin không được ghi nhận thành tính năng Nhắc hẹn. Cần kiểm thử resize/upfile thực trên Windows. |
| [PR #40 — giờ theo OS](https://github.com/ctdoteam1110/ctdozlite/pull/40) | Đã có | `clock::local_datetime/message_time`, day divider/sidebar và giờ mới gửi dùng giờ OS; giữ nguyên epoch. Có test local time, giây/mili giây, mốc ngày và timestamp lỗi. |
| [PR #47 — render theo vùng nhìn](https://github.com/ctdoteam1110/ctdozlite/pull/47) | Đã có phần lớn, còn thiếu | 150 tin mới nhất, nới cửa sổ khi tải thêm, sidebar `ui.is_rect_visible`, thumbnail decode ngoài UI và texture LRU; giữ cửa sổ khi quote. Timer 1,2 giây còn; sender-avatar vẫn tìm tuyến tính theo tên. Không dùng số RAM/CPU upstream làm số đo của fork. Nguồn upstream mới nhất cũng không còn mọi chi tiết như mô tả PR ban đầu. |
| [PR #55 — model/trạng thái tin](https://github.com/ctdoteam1110/ctdozlite/pull/55) | Đã có và nối runtime | `message_model`, `reactions`, `unread`, durable state và `SidebarPresentation`; DB composite identity, replay/recall/delivery adapters và các regression đã chạy. Không chỉ thêm pure module rồi để không dùng. |
| [PR #52 — session nguyên tử](https://github.com/ctdoteam1110/ctdozlite/pull/52) | Đã có; Windows thêm DPAPI | `Session::save_to_file` → DPAPI → `storage::atomic_write`: temp riêng, sync, atomic replace Windows; lỗi giữ đích cũ. Test atomic/rollback và native CI. |
| [PR #53 — giới hạn/cô lập media](https://github.com/ctdoteam1110/ctdozlite/pull/53) | Có nền và adapter ảnh; chưa đồng nhất mọi loại media | `media_resources` có key account/quality, bounded admission, private cache/prune 512 MiB, texture LRU 128 MiB, cancellation/partial tests; GUI ảnh đã dùng. Video viewer cache mới ở `*.video-cache` chưa đi qua cùng budget/prune; không khẳng định mọi cache media đều có trần 512 MiB. File người dùng chủ động lưu không thuộc cache bị prune. |

## Kiểm chứng và phần cần tiếp theo

Bộ test tự động dùng dữ liệu giả, không tài khoản thật. Bổ sung test bytes GIF/PNG, khóa texture chỉ nhận đúng pack, tài khoản đăng xuất, chặn synthetic category, thumbnail không mạng/cache, draft giữ owner/recipient/text và từ chối giờ sai, acknowledgement đúng request ID mới xóa ô soạn, lỗi ghi giữ nội dung để retry. Thêm smoke GUI schedule lớn/nhỏ và Ami vào build script Windows ARM64.

Các phần chưa đủ được ghi nhận qua kiểm tra mã, chưa tự mở rộng thành thay đổi runtime trong lần đối chiếu: bỏ repaint định kỳ, tối ưu tra avatar theo định danh, xóa conversation đúng nghĩa, reminder riêng và budget cache video. UniKey/điện thoại/resize/upload thực cần kiểm thử tương tác; CI xanh không thay thế các luồng đó.

Kết quả host sau bổ sung: **362 test passed, 0 failed, 6 ignored**; `cargo clippy --all-targets -- -D warnings` và build debug đạt. Smoke Xvfb đã kiểm hộp hẹn giờ lớn/nhỏ, bộ GIF và nút Gửi ở cửa sổ 480×360, DPI 150%. Không dùng kết quả này để khẳng định đã chạy UniKey hoặc sync điện thoại thật.
