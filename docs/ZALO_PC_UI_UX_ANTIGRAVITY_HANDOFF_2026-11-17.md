# Bàn giao Antigravity — sửa UX/UI theo Zalo PC gốc

Ngày: 17/11/2026. Bản tham chiếu: Zalo PC **26.11.12**, tài khoản đã đăng nhập trên máy người dùng. Snapshot mã được đọc: `9339a75` (`v0.016-arm`).

## 1. Mục tiêu và cách dùng

Mục tiêu người dùng: giống Zalo PC gốc nhất có thể về giao diện và trải nghiệm thao tác, đặc biệt sửa các lỗi đang gây khó chịu. Đây là tài liệu triển khai tiếp theo, không phải chứng nhận fork đã đạt parity. Chưa có sửa mã sản phẩm, build hay nghiệm thu fork trong đợt khảo sát này.

Antigravity bắt đầu bằng kiểm tra branch, `git status`, HEAD và mã hiện tại; số dòng/hành vi trong tài liệu có thể thay đổi. Đọc `AGENTS.md`, `docs/ZALO_REFERENCE_ANALYSIS.md`, các standards liên quan, `docs/ZALO_GUI_UX_OVERHAUL_PLAN.md` và `docs/GUI_UX_HANDOVER_AI_GUIDE.md`. Năm phase cũ được ghi completed không đồng nghĩa các yêu cầu bên dưới đã hoàn thành. Không đổi trạng thái phase cũ hoặc ghi đã merged khi chưa có bằng chứng.

Thực hiện từng gói nhỏ theo thứ tự trong mục 5. Giữ Rust/egui, WorkerCmd/WorkerEvent, account isolation và luồng protocol hiện có. Không thêm nút giả, không bỏ payload, không đổi hợp đồng server từ suy luận giao diện. Không tự commit/push/tag/release chỉ vì đọc tài liệu này.

## 2. Bằng chứng và giới hạn

| Nhãn | Nguồn | Giá trị chứng minh |
|---|---|---|
| UI-LIVE | Quan sát và tương tác trực tiếp Zalo PC 26.11.12 ngày 17/11/2026 | Bố cục, điều hướng, menu, biểu mẫu và trạng thái đã thấy; **không phải capture traffic E1** |
| CODE | Đọc mã tại snapshot trên | Cơ chế đang viết trong mã; chưa tự chứng minh hành vi binary người dùng đang chạy |
| USER | Người dùng báo lỗi fork | Dấu vuông glyph; hover nền xuyên Cài đặt fullscreen; hover khó dùng; nhiều vùng ép chat khi cửa sổ hẹp |
| UNKNOWN | Chưa quan sát/đo hoặc chưa test | Phải ghi rõ; không điền giả định thành thông số Zalo gốc |

Mẫu cá nhân: Henry, hai tin “Alo”, “123”, một link và file PDF, lịch sử cuộc gọi. Mẫu nhóm: PTSP - Đảo Hải Tặc (5 thành viên, tài khoản đang có quyền trưởng nhóm). Mẫu avatar: nhóm 3/4 người và nhóm đông trong sidebar/Danh bạ. Chỉ lưu mô tả cần thiết; không đưa ảnh chat riêng, URL mời, số điện thoại, UID/session/DB vào Git.

Đã xem: toàn chiều cuộn bảng thông tin Henry/PTSP; Nhóm chung; thành viên và menu thành viên; quản lý nhóm; nhắc hẹn; bốn tab bảng tin; biểu mẫu ghi chú/bình chọn; ba tab kho; ghim đầu chat; Sticker/Emoji/GIF; định dạng; tìm kiếm toàn cục; phân loại; 10 trang Cài đặt; một lần Đồng bộ tin nhắn; hover/reply/chia sẻ; cửa sổ hẹp khoảng 551×682 và cửa sổ rộng.

Không gửi tin/reaction/forward, không tạo note/poll/reminder hoặc đổi quyền/settings. Một lần đồng bộ đã chạy và hiện thành công. Chưa khảo sát mọi modal lồng nhau, trạng thái lỗi server, media viewer đầy đủ, dark mode gốc, vai trò thành viên thường hay ngưỡng responsive chính xác. Không có bộ ảnh baseline được lưu trong repo; khi nghiệm thu phải chụp lại hai ứng dụng cùng kích thước/DPI, dùng dữ liệu được phép và che thông tin riêng.

## 3. Bảng tổng hợp việc phải làm

P0: lỗi cản trở đọc/thao tác hoặc có nguy cơ gửi nhầm. P1: sai khác lớn trong sử dụng thường xuyên. P2: chiều sâu và tinh chỉnh sau nền tảng. Ưu tiên là đề xuất triển khai, không phải mức nghiệm thu đã đạt.

| ID | Ưu tiên | Khu vực / bằng chứng | Hiện trạng hoặc sai khác | Hướng sửa / kết quả cần đạt |
|---|---|---|---|---|
| UX01 | P0 | Glyph/icon/emoji — USER, CODE | Nút, tên người dùng có emoji/icon text hiện dấu vuông. Font hiện có Be Vietnam Pro, font Windows, Lucide; chưa chứng minh toàn pipeline glyph | Sửa rendering/fallback, tách icon UI khỏi văn bản, kiểm tra chuỗi emoji hoàn chỉnh; không sửa/xóa tên người dùng |
| UX02 | P0 | Modal — USER, CODE | Cài đặt phủ tối nhưng rê phần chat còn lộ vẫn bật hover; backdrop và toolbar đều Foreground; hover đọc tọa độ trực tiếp | Modal chặn input nền và dừng hover/tooltip/picker/context menu/shortcut nền; xóa hover khi mở modal |
| UX03 | P0 | Chia sẻ/forward — UI-LIVE, CODE | Fork chọn một hàng người nhận là dispatch Forward ngay | Chọn người nhận chỉ cập nhật selection; nút Chia sẻ xác nhận mới dispatch; hỗ trợ preview, hủy và kết quả gửi |
| UX04 | P1 | Responsive — UI-LIVE, USER, CODE | Fork dùng breakpoint 780/620, sidebar avatar thường trực; cột bên phải vẫn có thể ép chat ở cỡ vừa | Hẹp: danh sách **hoặc** chat, có Back; thông tin phủ chat; rộng mới dock cột khi đủ chiều rộng chat |
| UX05 | P1 | Hover action — UI-LIVE, CODE | Fork hộp bốn nút ở mép trên bubble, vùng giữ hover rộng, dùng any_popup_open toàn cục | Cụm ba nút Reply/Chia sẻ/Thêm gần đáy cạnh bubble; ổn định target/corridor/hide; reaction riêng |
| UX06 | P1 | Reaction — UI-LIVE | Gốc có nút riêng góc dưới bubble; giữ chuột mở sáu cảm xúc 👍 ❤️ 😆 😮 😢 😠 | Picker neo đúng tin, không đổi layout, không nhảy mục tiêu; phân biệt nút thao tác và pill reaction hiện có; xác minh thêm click nhanh/clear |
| UX07 | P1 | Reply — UI-LIVE | Gốc dải quote gọn trong composer có tên, text/thumbnail, vạch xanh và hủy | Khớp dải reply, focus editor, giữ draft; đi tới tin gốc đúng identity khi có dữ liệu |
| UX08 | P1 | Composer thường/định dạng — UI-LIVE | Gốc đổi chế độ editor, vùng soạn cao hơn; toolbar định dạng ở đáy. Fork chưa được xác minh có editor tương đương | Dựng hai chế độ, selection/style spans, clipboard/IME/undo; không coi khả năng render rich text là khả năng soạn/gửi |
| UX09 | P1 | Sticker/Emoji/GIF — UI-LIVE, CODE | Fork tabs Popular/Onion/Search/Store khác bảng nổi chung ba tab gốc | Picker neo composer: Sticker/Emoji/GIF, gần đây, search, pack/category rail ở đáy, mở rộng; tái dùng backend |
| UX10 | P1 | Tìm kiếm trái trên — UI-LIVE | Gốc thay nội dung sidebar tại chỗ; ô trống có gần đây, Nhắc bạn/Biểu cảm; có từ khóa thì Tất cả/Liên hệ/Tin nhắn/File | Tách trạng thái search khỏi lọc tên chat; nhóm kết quả, highlight, count, tải tiếp, xóa/đóng/quay lại |
| UX11 | P1 | Phân loại — UI-LIVE | Gốc checkbox nhiều thẻ màu và quản lý thẻ dạng modal | Lọc/gán thẻ, tên/màu/thứ tự lấy từ tài khoản; tạo/sửa/xóa/sắp xếp và đồng bộ theo contract đã có bằng chứng |
| UX12 | P1 | Overview cá nhân/nhóm — UI-LIVE, CODE | Fork dùng 4 quick actions cả cá nhân; nút Cài đặt dẫn bảo mật | Cá nhân 3 nút; nhóm 4 nút có Quản lý nhóm; đúng title, accordion, thứ tự và đường dẫn |
| UX13 | P1 | Nhóm chung — UI-LIVE | Gốc trang riêng: Tất cả/Nhóm của tôi, chọn nhiều, thêm vào nhóm | Nối dữ liệu thật, avatar/count; chọn và confirm rõ; không chỉ mở group panel chung |
| UX14 | P1 | Trang con drawer — UI-LIVE, CODE | Enum hiện có Overview/Members/Board/MediaStorage/Search/Security; chưa tách đủ | Điều hướng có Back, đúng thread, giữ tab/filter/scroll; thêm các view cần thiết mà không tạo window rời tùy tiện |
| UX15 | P1 | Bảng tin nhóm — UI-LIVE, CODE | Fork gộp Bảng tin & Nhắc hẹn, tabs Nhắc hẹn/Tin ghim server | Tách Nhắc hẹn; bảng tin 4 tab Tất cả/Tin ghim/Ghi chú/Bình chọn, nút +, thẻ và Xem tin nhắn gốc |
| UX16 | P1 | Thành viên/quản lý nhóm — UI-LIVE | Gốc có trang quản lý riêng và menu theo quyền; fork có backend một phần | Nối đúng vai trò, menu member, quyền, link, approval, blocked members, trưởng/phó; không giả thành công |
| UX17 | P1 | Kho ảnh/file/link — UI-LIVE, CODE | Fork trang kho dùng lưới 4 cột và dữ liệu shared đang có; thiếu UI filter tại chỗ | Overview ảnh 4 cột; **trang kho ảnh 3 cột ở mẫu đã thấy**, nhóm ngày; Files/Links search/filter; chọn nhiều, paging và trạng thái |
| UX18 | P1 | Tìm trong chat — UI-LIVE, CODE | Fork tìm tin đang nạp rồi mở toàn DB riêng | Tìm thống nhất trong drawer, filter người gửi/ngày; hiển thị scope/partial/loading/error đúng, jump đúng message |
| UX19 | P1 | Đồng bộ — UI-LIVE, CODE | Fork panel dưới màn hình cao khoảng 92; gốc kết quả ở đầu sidebar | Banner dưới hàng filter sidebar, không bóp composer; chi tiết kỹ thuật theo yêu cầu; trạng thái thật, không fake % |
| UX20 | P1 | Avatar — UI-LIVE, CODE | Danh bạ fork vẽ initials; renderer composite có fallback tách tên nhóm và ô TV/N | Dùng renderer chung + member avatar thật; 3 tam giác, 4 ô, đông hơn 3 ảnh + tổng số; ưu tiên ảnh nhóm riêng |
| UX21 | P1 | Cài đặt — UI-LIVE, CODE | Gốc 10 trang; enum fork 7 tabs | Khớp sidebar, cards, control, scroll; đặt feature hiện có đúng trang; phân biệt local/server settings |
| UX22 | P2 | Nhắc hẹn — UI-LIVE, CODE | Gốc giờ nhanh/lịch/lặp/RSVP; fork có ô ngày giờ kỹ thuật, repeat chưa đầy đủ | Modal nội dung/link, 15/30 phút, 9h mai/Khác, lịch, lặp; group cards tham gia/từ chối/thay đổi |
| UX23 | P2 | Note/poll — UI-LIVE, CODE | Backend có một số field nhưng UI drawer chưa tương đương | Note pin; poll deadline/multi/add options/hide results/hide voters; cards votes/end/change; contract cần evidence |
| UX24 | P2 | Ghim đầu chat — UI-LIVE | Gốc danh sách mở rộng dưới header, backdrop vùng chat, menu item và sang bảng tin | Tách ghim hội thoại/tin/đánh dấu; đúng count, target và overlay input |
| UX25 | P2 | Bubble/media/menu — UI-LIVE | Đã có nền nhưng chưa visual acceptance | Font, padding, grouping, timestamp/receipt, reaction pill, file/link/call cards, menu theo type/quyền; không nhồi engineering labels |
| UX26 | P2 | Media viewer/Windows | Album position còn TODO; autostart disabled; một số schedule/forward limits | Hoàn thiện sau core UX; kiểm tra native clipboard, viewer, OS integrations riêng; không cam kết parity chưa test |

## 4. Hướng dẫn kỹ thuật bắt buộc

### A. Sửa dấu vuông trước khi chỉnh mỹ thuật

Đây là lỗi do người dùng báo, chưa được tái hiện bằng binary fork trong khảo sát. Không kết luận mọi dấu vuông cùng một nguyên nhân.

1. Tạo màn/fixture glyph dùng dữ liệu giả: tiếng Việt có dấu tổ hợp; tên `Người dùng ❤️`, `👨‍👩‍👧‍👦`, `👍🏽`, cờ, variation selector, ký tự CJK; toàn bộ icon PUA đang dùng. Không dùng thông tin người dùng thật trong test.
2. Rà `theme.rs`: `build_fonts`, `select_font`, family mapping sau thay font/theme; `draw_lucide_icon` hiện dùng `FontId::proportional` và các hằng PUA. Xác minh codepoint đúng **phiên bản lucide.ttf** đang bundle; kiểm tra font bị thay/lost sau reload. Việc insert font không chứng minh glyph chọn đúng.
3. Icon UI dùng painter/vector hoặc family icon chỉ định rõ, với fallback nhìn thấy được. Không trông chờ emoji OS để vẽ nút chức năng; không dùng family icon cho tên/text người dùng. Giữ accessible label/tooltip tiếng Việt.
4. Emoji trong tên/tin cần pipeline fallback hoặc asset renderer phù hợp egui hiện tại. Phải xử lý grapheme/ZWJ/skin tone/VS, baseline và crop; **thêm Segoe UI Emoji vào danh sách font không tự chứng minh egui render được emoji màu/chuỗi ghép**. Không thay emoji bằng dấu hỏi hoặc cắt bỏ ký tự trong dữ liệu.
5. Fallback ASCII có ý nghĩa chỉ dành cho control nếu asset lỗi; bảo toàn nguyên văn user text. Crop/truncate theo grapheme và chiều rộng đo được, không cắt giữa chuỗi.
6. Kiểm tra Windows 11 ARM64 native, font setting, light/dark, 100/125/150/200% DPI; test cả startup và đổi theme/font. Font proprietary chỉ đọc từ Windows khi phù hợp, không đóng gói trái phép.

### B. Modal và hover

Điểm neo: `modals.rs::render_modal_backdrop`, `workspace.rs::render_workspace_overlays`, `messages.rs::hover_message_actions`.

Backdrop hiện là Foreground Area nhận click, còn hover đọc `pointer.hover_pos()` + geometry; modal không có guard rõ ở hàm hover. Thiết kế input gate chung cho modal stack và overlay thay vì vá riêng Cài đặt. UI nền không được emit command hoặc xử lý shortcut khi bị chặn. Chọn cơ chế modal phù hợp phiên bản egui pinned; kiểm tra layering và hit test thật, không chỉ vẽ nền tối. Không chặn nút/popup thuộc chính modal.

Reset active hover/reaction/context menu khi mở modal, đổi account/thread, cuộn target khỏi viewport hoặc message biến mất. Giữ identity message ổn định; không giữ bằng index list. Thay `any_popup_open()` toàn cục bằng ownership rõ. Hành lang chuyển chuột chỉ nối bubble/nút/picker cần thiết; tránh hợp toàn row thành vùng giữ lớn. Chốt delay sau đo bản gốc, chưa có số ms chuẩn.

Zalo gốc đã thấy cụm Reply/Chia sẻ/Thêm gần **đáy** cạnh bubble: nhận ở phải, gửi ở trái; reaction riêng góc dưới. Kiểm tra text ngắn, link/ảnh cao, album và mép viewport. Không làm layout đổi chiều cao khi hover.

### C. Forward và reply

`modals.rs::render_forward_dialog`: `chosen` hiện dẫn trực tiếp tới `WorkerCmd::Forward`. Thay bằng selection + preview + xác nhận. Gốc có search, tabs Gần đây/Nhóm trò chuyện/Bạn bè, Phân loại, checkbox nhiều đích, preview và nút Chia sẻ. Chọn hàng không gửi; cancel không gửi; double click/Enter không được vô tình xác nhận. Nếu nhiều đích, báo kết quả từng đích và không retry mutation mù. Reply giữ quote identity/type và draft theo account/thread; không tự gửi khi focus hoặc đổi mode.

### D. Responsive

Mẫu UI-LIVE hẹp ~551×682 (đơn vị ảnh capture, không phải breakpoint CSS/DPI đã chốt): nav rail + list; mở thread thay list bằng chat có Back. Info drawer phủ vùng chat, không reflow chat thành dải nhỏ. Khôi phục rộng trở về multi-column.

Mã hiện tại: `screen_w < 780` compact, `< 620` narrow; narrow vẫn giữ sidebar 68; dock drawer từ 780 trở lên. Đề xuất layout modes Wide/Medium/Narrow dựa **ngân sách chiều rộng còn lại**, font scale/DPI và min usable chat. Ngưỡng cụ thể phải đo, không nhận các số cũ làm chuẩn. Wide dock ba vùng; medium ưu tiên chat, info overlay; narrow list hoặc chat. Info button còn truy cập được khi compact. Bảo toàn thread/draft/scroll/view; overlay chặn nền theo B.

### E. Avatar và dữ liệu

`theme.rs::paint_user_avatar` đã có composite; `contacts.rs` nhiều chỗ vẫn vẽ circle+initials trực tiếp. Thống nhất renderer trên sidebar, contacts, header, drawer và pickers. Tìm data producer: khảo sát Rust chỉ thấy parser `multi:`, chưa thấy producer; xác minh lại trước sửa. Không tạo avatar thành viên giả bằng tên nhóm hoặc placeholder TV/N. Tách total count có thẩm quyền khỏi số member đã tải; load async/cache scoped account, invalidate theo update. Thứ tự/chọn thành viên, case 2 người và 99+ của **bản gốc** vẫn UNKNOWN; không chứng nhận từ comment code.

### F. Composer, tìm kiếm, thẻ và kho

Composer thường có utility toolbar trên editor; chế độ định dạng mở editor cao hơn và toolbar đáy: bold/italic/underline/strike, cỡ/màu chữ, clear format, bullet/number list, indent/outdent, undo/redo, mở rộng. Gốc hiển thị Ctrl+Shift+X. Phải thử selection, Unicode/IME, paste rich text, undo, switch mode và draft; outbound formatting chỉ đổi khi có E1/E2 hợp lệ về contract.

Picker chung Sticker/Emoji/GIF neo nút, recent/search/category/pack rail đúng chỗ; không tự gửi khi search hoặc hover. Tìm toàn cục thay sidebar tại chỗ; giữ selected chat, cancel stale result theo generation/account/thread. Phân loại không hardcode bộ tên thẻ mẫu. Kho phải có nhóm ngày, filter, tải tiếp, loading/empty/error; không gọi tập message đang nạp là toàn lịch sử.

### G. Cài đặt và đồng bộ

| Trang gốc | Nội dung đã thấy cần đối chiếu |
|---|---|
| Chung | Ngôn ngữ, autostart, ghi nhớ đăng nhập |
| Tài khoản & bảo mật | Khóa màn hình, 2FA; không tạo công tắc giả hoặc tự đổi bảo mật khi test |
| Riêng tư | Birthday, online/seen, quyền nhắn/gọi, block list, tìm qua SĐT, nguồn kết bạn |
| Đồng bộ & sao lưu | Hai card riêng: điện thoại → PC và sao lưu text PC để khôi phục điện thoại |
| Quản lý dữ liệu | Storage tổng/location, media/cache/system/account khác; xóa phải confirm đúng scope |
| Giao diện | Sáng/Tối/Hệ thống, avatar làm nền chat |
| Thông báo | Bật/tắt notification và âm thanh |
| Tin nhắn | Quick messages, auto-reply, hidden PIN, Ưu tiên/Khác, download directory, paste Excel hình/chữ/hỏi, double-click reply, preload <30MB, file thumbnail, typing, mute video |
| Cuộc gọi | Notification, mic/speaker/camera, policy khi thiếu thiết bị |
| Tiện ích | Capture shortcuts có/không cửa sổ Zalo, gợi ý Sticker/Link/@/GIF |

Không chuyển các công cụ chẩn đoán Sync2/V1/checkpoint thành nội dung mặc định làm người dùng khó hiểu; giữ khả năng chẩn đoán cần thiết sau mục chi tiết. Không làm mất nguyên tắc phone approval và account isolation.

Một lần click Sync thực tế đã đóng Settings và hiện banner xanh có check + “Đồng bộ tin nhắn thành công” + × ngay dưới filter sidebar; banner sau đó không còn hiển thị. **Chưa đo timeout, không thấy progress %, chờ phone hoặc lỗi vì hoàn thành quá nhanh.** Fork hiện `TopBottomPanel::bottom("main_sync_status")` cao 92 (compact 34). Chuyển summary về sidebar, detail khi yêu cầu; progress/phase lấy từ worker thật. Hủy/partial/error không thành success. Cần live capture thêm trạng thái đang chạy, mất kết nối, chờ phone, cancel trước khi gọi là parity hoàn chỉnh.

## 5. Thứ tự triển khai và checkpoint

| Gói | IDs | Sản phẩm review được |
|---|---|---|
| A | UX01 | Glyph rendering + fixture + ảnh native không dấu vuông ở controls và tên mẫu |
| B | UX02–03 | Modal input gate; forward hai bước, không gửi khi chọn/hủy |
| C | UX04–07 | Layout modes, hover/reaction/reply ổn định; kiểm tra chuột thật |
| D | UX08–11 | Composer/picker/search/labels; end-to-end controls |
| E | UX12–18,20 | Personal/group views, group navigation, avatar, kho và search |
| F | UX19,21–26 | Settings/sync, feature depth và visual polish; phần protocol cần evidence riêng |

Ưu tiên phần thuần UI có backend sẵn; feature chưa đủ contract ghi BLOCKED-EVIDENCE theo từng mục, tiếp tục phần độc lập. Đừng mở rộng thành audit protocol toàn dự án. Nếu thay kiến trúc/behavior trái ADR, nêu rõ và dùng quy trình ADR hiện hữu.

## 6. Ma trận nghiệm thu

| Ca | Điều kiện / thao tác | Kết quả bắt buộc |
|---|---|---|
| V01 | Tên giả có Việt/CJK/emoji ZWJ/skin tone + toàn bộ icon; đổi font/theme/DPI | Không tofu; không mất/chia sai grapheme; icon đúng; data gốc không bị sửa |
| V02 | Fullscreen mở Settings; rê/click/right-click/double-click/wheel vùng chat lộ | Không hover/tooltip/menu/scroll/command nền; modal vẫn dùng được |
| V03 | Mở modal khi reaction picker đang mở; close, switch account/thread | Không popup/target cũ; không emit mutation muộn |
| V04 | Hai tin Alo/123 liên tiếp và ảnh/link cao; đi chuột bubble→nút→picker→ngoài | Target đúng, không flicker/jump, không tăng chiều cao hàng, không mắc hover |
| V05 | Chia sẻ chọn một/nhiều đích rồi Hủy; sau đó chọn và confirm bằng fixture | Hủy/chọn không gửi; confirm gửi đúng đích đúng một lần; partial/error rõ |
| V06 | Resize rộng→vừa→hẹp→rộng, info mở/đóng, font scale cao | Chat đọc được; hẹp list/chat có Back; info overlay; draft/scroll giữ nguyên |
| V07 | Soạn rich text, Vietnamese IME, paste, chọn text, undo/redo, reply | Không gửi ngoài ý muốn, style/draft đúng; payload cần fixture nếu đổi |
| V08 | Picker search/category/recent, search toàn cục, switch thread khi loading | Không gửi khi hover/search; không stale results/cross-account data |
| V09 | Avatar ảnh riêng, 3/4/>4 thành viên, thiếu ảnh, đổi ảnh/member | Render nhất quán mọi surface; count thật; không suy thành viên từ tên |
| V10 | Group/person, admin/member; board/note/poll/reminder | Đúng view/quyền, loading/empty/error, payload không bị bỏ |
| V11 | Kho/search nhiều ngày, dữ liệu chưa nạp, paging/filter | Scope rõ, không false-empty hoặc false-complete, jump đúng tin |
| V12 | Sync success/partial/failure/cancel/wait-phone; đóng settings | Summary không bóp chat; state từ worker; không fake %, không success sớm |

Chạy test tập trung cho input gating, command emission/forward, identity và layout budget; không chỉ test hằng tọa độ hoặc snapshot mirror implementation. Theo AGENTS trước commit/push chạy `powershell scripts/audit-prepush.ps1` hoặc PATH cargo + `cargo check --locked`; với thay đổi GUI chạy thêm `cargo fmt --all -- --check`, `cargo test --locked` và `cargo clippy --all-targets -- -D warnings` theo toolchain pinned/standards. Báo đúng lệnh và kết quả thực chạy; không tự đổi toolchain sang stable từ hướng dẫn cũ.

Compile/unit/fixture không thay native Windows ARM64 acceptance hoặc live Zalo interoperability. Visual comparison phải cùng viewport/DPI/theme/data tương đương; ghi ảnh before/after đã che riêng tư. Mỗi ID báo riêng: chưa làm / code xong / check pass / native verified / live verified (nếu cần). Không gộp thành “100% giống Zalo”.

## 7. Mẫu báo cáo Antigravity sau mỗi gói

| ID | Thay đổi và file | Nguồn so sánh | Check đã chạy | Native/visual evidence | Khoảng trống còn lại |
|---|---|---|---|---|---|
| UXxx | ... | UI-LIVE tài liệu này; thêm re-notes nếu contract đổi | Lệnh + kết quả | Thiết bị, DPI, viewport, before/after | UNKNOWN cụ thể |

Khi protocol-adjacent, ghi rõ `Parity with Zalo PC/Web (source: ...)`, `Fork deviation because ...` hoặc `Unknown — AMBIGUOUS` theo AGENTS. Tài liệu này chỉ xác minh UI quan sát được, không thay nguồn schema/traffic. Kết thúc bàn giao phải cho biết chính xác mục nào còn chưa native-tested, không tự commit/push hoặc sửa matrix thành merged.
