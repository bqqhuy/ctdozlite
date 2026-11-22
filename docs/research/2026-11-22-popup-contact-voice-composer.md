# Popup, tên contact, voice và composer — bản sửa 22/11/2026

Trạng thái: đang kiểm chứng working tree; không phải chứng nhận hoàn thành P1–P5.
Không commit/push/tag trong đợt này.

## Nguồn đối chiếu

- PC 26.11.12: bundle chính và lazy `main-startup` trích từ ASAR cài trên máy.
- [Phonebook/alias E2](../../re-notes/69-contact-names.md).
- [Cấu hình composer E2](../../re-notes/70-composer-remote-config.md).
- [Voice/player/AI E2](../../re-notes/47-pc-native-voice-playback-ai.md).
- USER: composer thường tự nở tối đa 5 dòng; rich có chế độ mở rộng chia chiều cao với timeline.
- UI-LIVE 22/11: mở My Documents trên PC gốc, focus editor, Ctrl+Shift+X, bấm mở rộng rồi Ctrl+Shift+X về thường. Rich rỗng có vùng nhập cao sẵn; toolbar ở đáy; editor và toolbar giữ đáy khi mở rộng. Không gửi tin trong thao tác so sánh này.

## Các nguyên nhân và thay đổi

| Mục | Nguyên nhân đã đọc trong mã | Thay đổi | Cần xác nhận native/live |
|---|---|---|---|
| Phân loại khóa workspace | Backdrop Foreground nằm trên Window Middle | Window cùng Foreground, render sau backdrop; test pointer click X và ungate | Mở/đóng khi đang tải, Esc, click X |
| Sidebar cắt mép khi search/sync | Header/banner intrinsic width lan sang available width của danh sách | Bounded child cho từng vùng; hàng dùng mép phải panel cố định; ellipsis theo pixel chừa giờ/badge | Idle → sync/search → clear ở DPI thực |
| Tên đúng DM, sai group | Thiếu phonebook; resolver trả tên wire trước contact | Nguồn alias và phonebook riêng; effective name theo UID; contact trước public wire name; event có owner/runtime | Contact có tên phonebook nhưng không có alias, tên người gửi/quote/reaction/member |
| Metadata đến chậm | Cả bảy nguồn và hidden join xong mới phát snapshot | Alias/phonebook phát riêng ngay khi xong, không chờ nguồn chậm | Startup có cache/không cache |
| Voice mở cửa sổ ngoài | Bubble chuyển DownloadMedia play sang player ngoài | MF decode → PCM thật → CPAL trên worker; UI play/pause/resume/replay/progress/error | Voice đến và history thật, codec thật, thiết bị phát |
| AI voice | Payload/domain và parser push cũ không khớp implementation PC | `{url}` session AES; dynamic ai_features; req_id, err, has_more, convert_finish; bounded early-result buffer, runtime/target validation | Server acceptance, quota, kết quả và lỗi thực |
| Composer nhảy lên, không nở | Tính chiều cao phần dư bằng hằng, editor auto-shrink; phím newline mặc định không phải Shift+Enter | Bottom-anchored child, đo chiều cao frame, viewport cố định theo galley 1–5 dòng, Shift+Enter binding thật | IME, nhập/paste nhiều dòng, 1→5→6→xóa |
| Rich không có phản hồi trực quan | Chỉ options toàn draft; layouter chỉ mention | Style ranges UTF-16, chọn vùng/toggle/caret insertion, preview cùng styles gửi đi; plain tắt styles thật | Chọn/toggle, gõ tiếp, gửi và xem ở PC |
| Remote config bị bỏ | getServerInfo đã gọi nhưng không hydrate zcinput | Parse versioned bounded config, event owner/runtime; auth config đi qua Session/LoginSuccess | Server có/không cấp config, fresh login và restored login |

Rich toolbar trong đợt này thực hiện B/I/U/S; chưa được coi là tương đương toàn bộ bộ công cụ PC (font size, màu, list, indent, undo toolbar). Voice recorder/upload đi vẫn nằm ngoài phạm vi sửa voice đến/lịch sử đã được người dùng điều chỉnh.

## ZaloData (chỉ đọc)

Đã kiểm tra cấu trúc folder người dùng chỉ định và tên khóa của config.json, database-config.json, Local State, Preferences. Có SQLite/WAL, LevelDB/Chromium storage và cache. Không đưa DB, khóa mã hóa, contact, nội dung chat hay danh tính tài khoản vào repo.
Không thấy marker zcinput trong các cache nhỏ đã kiểm tra; đây không chứng minh cấu hình không nằm ở storage nén/khác. Chuỗi Q7+W/I9Fb trong bundle chứng minh cache remote config được đọc từ RAM; không dùng dữ liệu tài khoản để giả lập cấu hình server.

## Kiểm chứng

- Thư viện sau lần tích hợp đầu: `cargo test --locked --lib -j1`: 1047 passed, 0 failed, 11 ignored.
- Test decoder Media Foundation thật trên máy Windows ARM dùng WAV giả lập 0,1 giây, 16 kHz: đạt. Không suy từ WAV fixture sang mọi codec voice server.
- Audit A yêu cầu sửa: metadata runtime fence; typing style tại caret; line-height đo/render; transcript muộn/partial; hidden styles khi tắt rich. Các sửa đã được đọc lại, Accept ở mức mã nguồn với điều kiện checks đạt.
- Audit B yêu cầu sửa race đăng ký AI ticket sau HTTP/đổi phiên; đã sửa bằng key runtime + req_id, kiểm tra phiên sau await và timeout full identity. Audit B đã đọc lại và Accept ở mức mã nguồn có điều kiện checks cuối đạt. Hai audit hiện đồng thuận phần mã trong phạm vi trên; không suy thành nghiệm thu native.
- Native build và nghiệm thu ghi bổ sung sau khi thực hiện, không đánh dấu trước.

## Tiếp tục kiểm chứng ngày 22/11/2026

Working tree trên `8e235f7`, tiếp tục chat `01a1f8a8-fbae-7d20-a7c1-8638838c83eb`.
Không commit/push/tag. Toolchain `1.99.1`, host `aarch64-pc-windows-msvc`.

- `cargo test --locked --lib -j 1`: **1048 passed, 0 failed, 11 ignored**.
  Bao gồm regression click X popup đang pending, bounds sidebar khi search/sync,
  composer giữ đáy và tăng dòng, phonebook/alias, decoder Media Foundation và
  runtime fence cho voice/AI. Không thay kết quả fixture bằng nghiệm thu server.
- `cargo clippy --all-targets --locked -j 1 -- -D warnings`: exit 0.
  Hai module test đã nằm cuối implementation; lỗi Clippy trong log phiên trước
  không còn tái hiện.
- Dọn một dòng trắng cuối `src/gui/account_state.rs` bằng `cargo fmt --all`.
  `cargo fmt --all -- --check` và `git diff --check`: exit 0 sau đó.
- Kiểm tra UI tương tác chưa chạy được: Computer Use không khởi tạo kernel vì
  `windows sandbox failed`; reset kernel và thử lại vẫn lỗi trước khi lấy cửa sổ.
  Không dùng lỗi công cụ này để xác nhận popup/search/Shift+Enter đã đạt native.
- Build debug ARM64 đầu tiên qua. Renderer smoke DX12 trên Qualcomm Adreno 8cx
  Gen 3: sidebar sync DPI 1 tạo ảnh, DPI 1,5 ban đầu exit 3 vì assertion nút gửi
  chạy trước bước đo chiều cao/discard. Chuyển assertion ra sau bước đo và chỉ
  kiểm tra pass được paint; vẫn fail nếu nút gửi vượt clip ở pass cuối.
- Ảnh smoke lộ filter strip chồng chữ. Bổ sung kiểm tra các vùng bấm không giao
  nhau vào regression sidebar; test đã fail tại `800@1 phase 0`. Sửa budget bốn
  control và căn cùng tâm dọc; chạy lại `cargo test --locked --lib composer_tests
  -j 1`: **10 passed, 0 failed**, gồm cả search/sync/clear ở nhiều DPI.
- Sau delta: Clippy all-targets với `-D warnings`, fmt check và diff check đều
  exit 0; `cargo build --locked --bin ctdozlite -j 1` exit 0. Binary mới:
  `target/debug/ctdozlite.exe` (debug ARM64, không phải release).
- Renderer smoke chạy bằng binary mới, `CTDOZ_DATA_DIR` riêng chứa dữ liệu giả:
  `sidebar-sync-1`, `sidebar-sync-1.5`, `composer-multiline`: **3/3 exit 0**,
  ảnh đã được xem. Artifact cục bộ tại `target/ui-continuation-20261122/`.
  Sidebar banner không có dải đen giữa panel; hàng filter không còn chồng chữ;
  composer ba dòng và nút gửi nằm trong viewport. Fixture `--ui-smoke-rich-text`
  tạo tin rich và draft ba dòng thường; ảnh đó không xác nhận thao tác bật rich.
- Finding còn mở từ ảnh DPI 1,5: tiêu đề/status chat hẹp chồng lên cụm action
  header. Không sửa lan sang header trong delta này và không chứng nhận toàn UI.
  Popup pointer/Shift+Enter/rich-expand có regression headless; chưa có lượt
  tương tác native mới. Phonebook theo dữ liệu thật, voice trên loa và AI qua
  server vẫn chưa được nghiệm thu. Không đóng các task thiết bị từ smoke fixture.
