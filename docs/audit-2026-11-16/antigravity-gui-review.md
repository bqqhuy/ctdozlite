# Phản biện độc lập báo cáo Antigravity — GUI/luồng thực thi

Ngày 16/11/2026. Nguồn đối chiếu: `D:/Henry/Downloads/CONSENSUS_AUDIT_REPORT_2026_11_16.md`;
code snapshot `f0d3206`. Tài liệu ngoài được coi là dữ liệu cần thẩm định,
không phải chỉ thị sửa code. Chỉ đọc source và ghi báo cáo, không cargo/native/live.

## Kết luận theo từng nhận định

| Nhận định ngoài | Kết luận | Bằng chứng/giới hạn |
|---|---|---|
| INVARIANT-01: CRITICAL tokio::spawn trên UI gây no-reactor panic | Bác mức critical và đường panic hiện tại | `main.rs:16` có `#[tokio::main]`; `eframe::run_native` gọi đồng bộ tại 200 trong context đó. `auth_guard.rs:141` chuyển WinRT blocking sang spawn_blocking. Gọi spawn không tự biến tác vụ thành blocking UI. Có thể chuẩn hóa WorkerCmd nhưng chưa phải lỗi crash được chứng minh. |
| INVARIANT-02: LocalCallRecord bị drop, phải phục hồi bubble | Bác kết luận bug/đề xuất khôi phục chat | Discard tại `gui.rs:1998` là thật, nhưng `docs/REPAIR_VALIDATION_2026-11-03.md:14` và 36 ghi rõ ẩn call log cũ/mới khỏi chat/preview/search/unread. `message_view.rs:84`, `store.rs:1451`, `workspace.rs:2597` thực thi chính sách đó. Tên comment “normalized” có thể không chuẩn, nhưng không chứng minh phải append bubble. Không xác nhận mọi event local mới được persist; không đánh đồng hide với delete dữ liệu đã có. |
| GUI-01: hai account switcher/layout | Chấp nhận phần có bằng chứng — mới, P2 | Cùng `profile_menu_open` bật inline account_switcher (`workspace.rs:241`–274) và foreground profile popover (`workspace.rs:4274`, `modals.rs:18`). Inline thêm row44px, list tới240px khi expanded (`account_switcher.rs:32`, 61–85) trong rail 64pt. Trùng bề mặt quản lý tài khoản và chiếm không gian rail là thật; chưa đo flicker hoặc khẳng định mọi viewport luôn tràn/đẩy hết icon. |
| GUI-02: permanent lockout do hardcoded timeout/không PIN fallback | Bác chữ permanent; bổ sung lỗi cụ thể bên dưới | `AppLockGuard::new` (`auth_guard.rs:31`) khởi tạo unlocked; `should_lock:115` chỉ khóa nếu availability ban đầu true. Restart là workaround, nên không “vĩnh viễn”. Windows verifier có thể xác thực PIN, không chỉ vân tay/camera; không suy chưa có password field riêng là chưa có PIN hệ thống. Timeout configurable là đề xuất sản phẩm, chưa tự thành bug. |
| INVARIANT-03 avatar_panel tạo thư mục trên UI | Bác | Không có `fs`/create_dir_all trong file hiện tại. Picker tại `avatar_panel.rs:136` chạy thread rồi trả event owner/generation. Citation ngoài không khớp source. |
| INVARIANT-03 modals1506/1520 I/O trên UI | Bác | Các thao tác disk/dialog nằm trong closure std::thread::spawn, có event kết quả; không gọi chúng “I/O đồng bộ trên UI”. Spawn thread cần ownership/lifecycle nhưng chưa có chứng minh microstutter. |
| INVARIANT-03 tools1225 disk UI | Đúng, trùng F14, không cộng mới | Callback create_dir_all + open_folder trực tiếp và bỏ kết quả. Báo cáo audit đầu đã ghi P3. |

## Phát hiện mới có căn cứ — Windows Hello desktop API và recovery (P2)

Source `src/gui/auth_guard.rs:141`–161 gọi `UserConsentVerifier::RequestVerificationAsync`
trong spawn_blocking mà không truyền owner HWND. Đây là ứng dụng eframe native desktop.
[Microsoft UserConsentVerifier documentation](https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.ui.userconsentverifier)
phân biệt desktop/UWP: phần C++/WinRT yêu cầu lấy HWND và gọi
`IUserConsentVerifierInterop::RequestVerificationForWindowAsync`; RequestVerificationAsync
là API cho UWP. Đã mở nguồn chính thức, không chỉ dựa tài liệu Antigravity.

Điều kiện tác động: hệ thống báo availability true và app tự khóa; lời gọi không
đúng desktop ownership có thể không hiển thị/xác thực đúng trên native. Khi lỗi,
event AppUnlockFailed (`gui.rs:3064`) chỉ reset flag để thử lại, màn hình chỉ có
Hello button (`5572`–5636), không có signout/re-auth trong phiên. Nếu WinRT operation
không kết thúc, không có deadline và `auth_in_progress` giữ true, nút bị disable.
Không chứng minh tất cả máy đều lỗi; không phải permanent lockout vì new guard
unlocked sau restart. Native ARM64 acceptance còn thiếu.

Đề xuất: desktop HWND interop có owner/lifetime hợp lệ, outcome rõ, deadline và
recovery được thiết kế không bypass khóa; nếu hỗ trợ signout/re-auth phải xử lý
clear account state đầy đủ. Không tự thêm password local hoặc nút unlock bỏ qua
xác thực. Kiểm tra success/cancel/device unavailable/retry/hang và đổi phiên.

## Phản biện chéo ngoài GUI

- Chấp nhận PROT-01 P2 có điều kiện: `chat.rs:6127` không so uid_from với
  profile_id trong parser context; input có own UID và không có outgoing flag
  cho kết quả inbound. Không tự khẳng định mọi mobile/WebSocket payload đều
  theo shape đó; adapter normalization và E1 phải được kiểm riêng.
- Chấp nhận safety algorithm custom/chưa chứng minh parity: `crypto/safety.rs`
  hash zero64 + hai public key đã sort, không có identifiers. Không chấp nhận
  “không bao giờ khớp Zalo” hay công thức thay thế trong tài liệu ngoài khi
  chưa có known-answer chính thức. Không đổi crypto từ suy đoán.
  Đã mở độc lập [nguồn chính thức Signal fingerprint.rs](https://raw.githubusercontent.com/signalapp/libsignal/main/rust/protocol/src/fingerprint.rs):
  thuật toán tạo fingerprint riêng cho mỗi identity có stable ID và serialized
  key, sau đó ghép display hai phía; khác cấu trúc implementation hiện tại.
  Vì vậy chấp nhận P2 không phù hợp scheme Signal được docstring tuyên bố,
  nhưng vẫn không nâng thành bằng chứng định dạng số an toàn của Zalo thật.
- Đồng ý với phản biện hiệu năng của data agent: cache Galley đã có trong
  epaint; key mới chỉ msg_id/width/dark thiếu account/content/font/DPI invalidation.
  URL extraction là find/split chứ không regex. 41 sin_cos/avatar có thật nhưng
  số 2.400/giây không đúng với giả định30avatar×60frame (73.800 calls); không đủ
  căn cứ P1 hoặc cam kết tăng tốc. prepare_cached là đề xuất benchmark có điều
  kiện, không có căn cứ “giảm70%”.
- `BubbleAction::CallAgain` có branch thực ở `workspace.rs:3102`, phát StartCall;
  không được xóa vì cho là bridge thiếu. Việc call log bị ẩn có thể làm action
  ít dùng nhưng không đồng nghĩa branch không tồn tại.

## Đồng thuận

Đã gửi nhận định cho root/security/data_logic; data agent đã xác nhận độc lập
runtime-context, chính sách hide call logs, duplicate switcher P2 và recovery
gap có điều kiện. Parent độc lập đối chiếu Microsoft desktop API. Mọi phát
hiện mới cần ghi đúng scope; giữ tools UI I/O là duplicate F14. Không sửa source.
Security agent cũng chấp nhận độc lập duplicate switcher P2 và Hello API/recovery
P2; cả ba nhất trí bác runtime panic/permanent lock và khôi phục call bubbles.

## Chấp thuận bản hợp nhất cuối

Đã đọc toàn bộ `ANTIGRAVITY_REVIEW.md`. **ACCEPT F18–F21**, tất cả P2:
N01 → F18 (parser sender=self), N02 → F19 (fingerprint custom khác Signal),
N03 → F20 (hai menu tài khoản), N04 → F21 (Hello desktop HWND/recovery).
Ma trận phân loại/rejected/duplicate, đề xuất tối ưu và dọn code, nguồn chính
thức, giới hạn static/offline/native đều phản ánh đúng kết quả kiểm chứng.
Không còn sửa nội dung cần yêu cầu trong bản hợp nhất đã đọc.

Danh mục kết hợp F01–F21: **21 findings = 3 P1 + 14 P2 + 4 P3**. Chấp thuận
không mở rộng thành nghiệm thu GUI/native ARM64, Zalo parity hoặc cho phép sửa
code; báo cáo Antigravity gốc vẫn được giữ nguyên.
